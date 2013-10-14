use common::*;
use monomial::{Monomial, DegLim, MaxMonDeg, MaxMonFactorDeg, domain_space_dims};
use polynomial::{PolyBorrowing};
use mesh::{Mesh, FENum, NBSideNum, NBSideInclusions, OShape, SideFace};
use weak_gradient::{WeakGradSolver, WeakGrad};

use std::vec;

/* Overview
 * --------
 * This module provides a type representing a basis for the space of piecewise polynomials
 * on the interiors and non-boundary sides of the elements of a given mesh, which satisfy
 * given constraints on the degrees of the constituent monomials or monomial factors.
 *
 * The elements of the basis are functions which are degree limit compliant monomials on
 * exactly one interior or non-boundary side in the mesh, and zero on all other parts of
 * the mesh. However, some degree-compliant side monomials must be omitted, depending on
 * the side, so that the elements supported on the side, and by extension the entire basis,
 * are linearly independent. The side's "dependent dimension" determines which monomials
 * supported on the side should be represented in the basis.
 *
 * Side Monomials and Side Dependent Dimensions
 * ----------------------------------------------------
 * Side supported basis element monomial sequences depend on the side *dependent dimension*, 
 * as declared by the mesh, which determines which monomials satisfying the degree limit can
 * be included to avoid having a linearly dependent set of basis elements. A side has 
 * dependent dimension of r if coordinate r can be expressed as a function (affine) of the
 * other coordinates on the side. The dependent dimension used for a side is chosen by the
 * mesh, and usually chosen from among multiple acceptable alternatives. A side declared to
 * have dependent dimension r by the mesh will host basis elements whose local monomials on
 * the side are constant (have exponent 0) in the r^th coordinate factor.
 *
 * Basis Layout and Enumeration
 * ----------------------------
 * The following rules determine the ordering of the basis elements:
 *   - All interior supported basis elements precede those supported on non-boundary sides.
 *   - Within these two main groups, the elements are grouped into smaller groups by the
 *     interior or side on which they are supported, arranged in ascending order by the
 *     mesh's numbering of the interiors or non-boundary sides.
 *   - Within the block of basis elements allocated to a single particular interior or side,
 *     the monomials representing the basis elements on the face are arranged in order of
 *     increasing exponent sequence, with exponents on lower dimension variables being more
 *     significant. So e.g. x^0 y^2 appears prior to x^1 y^1.
 *
 * Overview of Module Functions
 * ----------------------------
 * The main job of the module is to translate back and forth between "local" basis element
 * identifiers on the one hand, defined in terms of finite elements or shapes, their interior
 * or side faces, and a locally defined monomial or monomial number, on the one hand, and the
 * flat global enumeration of all basis elements on the other hand.
 * The following are the detailed functions of the module:
 *  - Retrieve the reference interior monomials sequence, used to define the basis elements
 *    supported on any interior.
 *  - Retrieve the reference side monomials sequence for any side of a finite element or
 *    oriented shape.
 *  - Determine the supporting finite element interior or non-boundary side for a given
 *    basis element number.
 *  - Determine the monomial used to define a given enumerated basis element, or likewise
 *    determine its face-relative monomial number.
 *  - Retrieve the basis element number given one of:
 *    -- a finite element interior and interior-relative monomial number
 *    -- a finite element side and side-relative monomial number
 *  - Retrieve the polynomial representing the full solution's restriction to a given finite
 *    element interior or side, given the full sequence of solution coefficients.
 *  - Retrieve the weak gradient of the basis element defined by a given monomial on the interior
 *    of a given oriented shape.
 *  - Retrieve the weak gradient of the basis element defined by a given side monomial on a given
 *    oriented shape side.
 *  - Retrieve the matrix of inner products between basis elements supported on any given finite
 *    element interior or side.
 *  - Provide an estimate and upper bound for the number of interacting basis element pairs.
 */

#[deriving(Eq,TotalEq,Ord,TotalOrd,Clone)]
pub struct BasisElNum(uint);

#[deriving(Eq,TotalEq,Ord,TotalOrd,Clone)]
pub struct FaceMonNum(uint);


// A type representing a basis for Weak Galerkin approximating polynomials on an arbitrary mesh.
struct WgBasis<Mon,Mesh> {
  
  // The mesh over which the Weak Galerkin basis is formed.
  mesh: ~Mesh,

  // Degree limits of the approximating polynomials.
  int_polys_deg_lim: DegLim,
  side_polys_deg_lim: DegLim,

  // Monomial sequence used to define basis shape functions on any one finite element interior.
  int_mons: ~[Mon],

  // This vector stores the sequences of monomials used to define basis shape functions supported on 
  // any single finite element side, arranged by side dependent dimension. Thus in the vector at
  // position r are the sequence of side-local monomials defining the shape functions in the basis
  // which are supported on any given side which the mesh declares to have dependent dimension r.
  side_mons_by_dep_dim: ~[~[Mon]], // side monomial sequences by side dependent dimension

  mons_per_fe_int: uint,
  mons_per_fe_side: uint,

  // Significant counts in the basis element enumeration.
  total_els: uint,
  num_int_els: uint, 
  
  // First side supported basis element. All preceeding basis elements are interior supported.
  first_nb_side_beln: BasisElNum,

  // Weak gradients generator.
  weak_grad_solver: ~WeakGradSolver<Mon>,

  // Pre-calculated weak gradients of basis elements supported on reference oriented shapes.
  int_mon_wgrads: ~[~[WeakGrad]],     // by fe oshape, then interior monomial number
  side_mon_wgrads: ~[~[~[WeakGrad]]], // by fe oshape, then side face, then side monomial number
}


impl <Mon:Monomial, MeshT:Mesh<Mon>> WgBasis<Mon,MeshT> {

  pub fn new(mesh: ~MeshT, int_polys_deg_lim: DegLim, side_polys_deg_lim: DegLim) -> ~WgBasis<Mon,MeshT> {
    
    let int_mons = Monomial::mons_with_deg_lim_asc(int_polys_deg_lim);
    
    let side_mons_by_dep_dim: ~[~[Mon]] = { 
      let mons_for_deg_lim: ~[Mon] = Monomial::mons_with_deg_lim_asc(side_polys_deg_lim);
      vec::from_fn(domain_space_dims::<Mon>(), |r|
        mons_for_deg_lim.iter().filter(|mon| mon.exp(Dim(r)) == Deg(0)).map(|m|m.clone()).collect()
      )
    };
    
    let mons_per_fe_int = int_mons.len();
    let mons_per_fe_side = side_mons_by_dep_dim[0].len();

    let num_int_els = mesh.num_fes() * int_mons.len(); 
    let total_els = num_int_els + mesh.num_nb_sides() * mons_per_fe_side;
    let first_nb_side_beln = BasisElNum(num_int_els);

    let mut wgrad_solver = {
      let k = match int_polys_deg_lim { MaxMonDeg(k) => k, MaxMonFactorDeg(k) => k };
      ~WeakGradSolver::new(MaxMonDeg(k-1), mesh)
    };

    let (int_mon_wgrads, side_mon_wgrads) = compute_wgrads(wgrad_solver, int_mons, side_mons_by_dep_dim, mesh);
    
    ~WgBasis {
      mesh: mesh,
      int_polys_deg_lim: int_polys_deg_lim,
      side_polys_deg_lim: side_polys_deg_lim,
      int_mons: int_mons,
      side_mons_by_dep_dim: side_mons_by_dep_dim,
      mons_per_fe_int: mons_per_fe_int,
      mons_per_fe_side: mons_per_fe_side,
      total_els: total_els,
      num_int_els: num_int_els,
      first_nb_side_beln: first_nb_side_beln,
      weak_grad_solver: wgrad_solver,
      int_mon_wgrads: int_mon_wgrads,
      side_mon_wgrads: side_mon_wgrads,
    }
  }

  /*
   Estimate an upper bound of the number of ordered triplets (bel1, bel2, fe) where bel1 and bel2 are basis elements
   supported on finite element fe. This function is intended to help callers allocate storage for data structures
   involving interacting basis element pairs, such as are used in the construction of sparse matrices.
  */
  pub fn ub_estimate_num_bel_bel_common_support_fe_triplets(&self) -> uint {
    let num_fes = self.mesh.num_fes();
    let int_int_interactions = num_fes * sq(self.mons_per_fe_int);
    range(0, num_fes).fold(int_int_interactions, |tot_interactions, fe| {
      let nb_sides = self.mesh.num_non_boundary_sides_for_fe(FENum(fe));
      let int_side_and_side_int_interactions = 2 * self.mons_per_fe_int * nb_sides * self.mons_per_fe_side;
      let side_sidemon_choices = nb_sides * self.mons_per_fe_side;
      let side_side_interactions = sq(side_sidemon_choices);
      tot_interactions + int_side_and_side_int_interactions + side_side_interactions
    })
  }
  
  /// Get the mesh for this basis.
  #[inline]
  pub fn mesh<'a>(&'a self) -> &'a MeshT {
    &*self.mesh
  }

  /// Determine whether a basis element is interior-supported.
  #[inline]
  pub fn is_int_supported(&self, i: BasisElNum) -> bool {
    *i < self.num_int_els
  }

  /// Determine whether a basis element is side-supported.
  #[inline]
  pub fn is_side_supported(&self, i: BasisElNum) -> bool {
    self.num_int_els <= *i && *i < self.total_els
  }

  /// Get the finite element number including the support for the given interior-supported basis element.
  #[inline]
  pub fn support_int_fe_num(&self, i: BasisElNum) -> FENum {
    assert!(self.is_int_supported(i));
    FENum(*i / self.mons_per_fe_int)
  }

  /// Get the non-boundary side number including the support for the given side-supported basis element.
  #[inline]
  pub fn support_nb_side_num(&self, i: BasisElNum) -> NBSideNum {
    assert!(self.is_side_supported(i));
    let sides_rel_ix = *i - *self.first_nb_side_beln;
    NBSideNum(sides_rel_ix / self.mons_per_fe_side)
  }

  /// Get information about the two finite elements which include the support of a given side-supported basis element.
  #[inline]
  pub fn fe_inclusions_of_side_support(&self, i: BasisElNum) -> NBSideInclusions {
    let supp_nb_side_num = self.support_nb_side_num(i);
    self.mesh.fe_inclusions_of_nb_side(supp_nb_side_num)
  }

  /// Get the reference monomial sequence defining the basis elements supported on any individual interior.
  #[inline]
  pub fn ref_int_mons<'a>(&'a self) -> &'a [Mon] {
    self.int_mons.as_slice()
  }

  /// Get the monomial sequence defining the basis elements supported on a particular finite element side.
  #[inline]
  pub fn side_mons_for_fe_side<'a>(&'a self, fe: FENum, side_face: SideFace) -> &'a [Mon] {
    let fe_oshape = self.mesh.oriented_shape_for_fe(fe);
    self.side_mons_for_oshape_side(fe_oshape, side_face)
  }

  /// Get the monomial sequence defining the basis elements supported on the given oriented shape side.
  #[inline]
  pub fn side_mons_for_oshape_side<'a>(&'a self, oshape: OShape, side_face: SideFace) -> &'a [Mon] {
    let side_dep_dim = self.mesh.dependent_dim_for_oshape_side(oshape, side_face);
    self.side_mons_by_dep_dim[*side_dep_dim].as_slice()
  }

  /// Get the number of basis elements supported on any single finite element interior.
  #[inline]
  pub fn mons_per_fe_int(&self) -> uint {
    self.mons_per_fe_int
  }

  /// Get the number of basis elements supported on any single finite element side.
  #[inline]
  pub fn mons_per_fe_side(&self) -> uint {
    self.mons_per_fe_side
  }

  /// Get the face-relative number of the monomial defining the given interior-supported basis element.
  #[inline]
  pub fn int_rel_mon_num(&self, i: BasisElNum) -> FaceMonNum {
    assert!(self.is_int_supported(i));
    FaceMonNum(*i % self.mons_per_fe_int)
  }

  /// Get the monomial defining the given interior-supported basis element.
  #[inline]
  pub fn int_mon(&self, i: BasisElNum) -> Mon {
    let rel_monn = self.int_rel_mon_num(i);
    self.int_mons[*rel_monn].clone()
  }

  /// Get the face-relative number of the monomial defining the given side-supported basis element.
  #[inline]
  pub fn side_rel_mon_num(&self, i: BasisElNum) -> FaceMonNum {
    assert!(self.is_side_supported(i));
    let nbsides_rel_ix = *i - *self.first_nb_side_beln;
    FaceMonNum(nbsides_rel_ix % self.mons_per_fe_side)
  }

  /// Get the basis element number for the given interior monomial number and finite element.
  #[inline]
  pub fn int_mon_el_num(&self, fe: FENum, monn: FaceMonNum) -> BasisElNum {
    BasisElNum(*fe * self.mons_per_fe_int + *monn)
  }

  /// Get the basis element number for the given monomial number and finite element side face.
  #[inline]
  pub fn side_mon_el_num(&self, fe: FENum, side_face: SideFace, monn: FaceMonNum) -> BasisElNum {
    let nb_side_num = self.mesh.nb_side_num_for_fe_side(fe, side_face);
    BasisElNum(*self.first_nb_side_beln + (*nb_side_num * self.mons_per_fe_side) + *monn)
  }


  /// Get the polynomial representing the passed full WG solution restricted to a particular finite element interior.
  pub fn fe_int_poly<'a>(&'a self, fe: FENum, sol_basis_coefs: &'a [R]) -> PolyBorrowing<'a,Mon> {
    let fe_first_int_beln = self.int_mon_el_num(fe, FaceMonNum(0));
    let fe_int_coefs = sol_basis_coefs.slice(*fe_first_int_beln, *fe_first_int_beln + self.mons_per_fe_int);
    PolyBorrowing::new(fe_int_coefs, self.int_mons)
  }

  /// Get the polynomial representing the passed full WG solution restricted to a particular finite element interior.
  pub fn fe_side_poly<'a>(&'a self, fe: FENum, side_face: SideFace, sol_basis_coefs: &'a [R]) -> PolyBorrowing<'a,Mon> {
    let fe_side_mons = self.side_mons_for_fe_side(fe, side_face);
    let fe_side_first_beln = self.side_mon_el_num(fe, side_face, FaceMonNum(0));
    let fe_side_coefs = sol_basis_coefs.slice(*fe_side_first_beln, *fe_side_first_beln + fe_side_mons.len());
    PolyBorrowing::new(fe_side_coefs, fe_side_mons)
  }


  // weak gradient accessors

  /// Get the weak gradient of the interior supported shape function defined by the given monomial on the interior of the given oriented shape. 
  pub fn wgrad_int_mon<'a>(&'a self, monn: FaceMonNum, oshape: OShape) -> &'a WeakGrad {
    &self.int_mon_wgrads[*oshape][*monn]
  }

  /// Get the weak gradient of the side supported shape function defined by the given monomial on the given side of the given oriented shape. 
  pub fn wgrad_side_mon<'a>(&'a self, monn: FaceMonNum, oshape: OShape, side_face: SideFace) -> &'a WeakGrad {
    &self.side_mon_wgrads[*oshape][*side_face][*monn]
  }

}  // WgBasis impl



// construction helpers


fn compute_wgrads<Mon:Monomial,MeshT:Mesh<Mon>>(wgrad_solver: &mut WeakGradSolver<Mon>,
                                                int_mons: &[Mon],
                                                side_mons_by_dep_dim: &[~[Mon]],
                                                mesh: &MeshT) -> (~[~[WeakGrad]], ~[~[~[WeakGrad]]]) {
  let mut int_mon_wgrads_by_oshape = vec::with_capacity(mesh.num_oriented_element_shapes());
  let mut side_mon_wgrads_by_oshape = vec::with_capacity(mesh.num_oriented_element_shapes());

  for os in range(0, mesh.num_oriented_element_shapes()) {
    let os = OShape(os);
    let side_mons_by_side = vec::from_fn(mesh.num_side_faces_for_shape(os), |sf| {
      let sf_dep_dim = mesh.dependent_dim_for_oshape_side(os, SideFace(sf));
      side_mons_by_dep_dim[*sf_dep_dim].as_slice()
    });
    
    let (int_mon_wgrads, side_mon_wgrads) = wgrad_solver.wgrads_on_oshape(int_mons, side_mons_by_side, os, mesh);

    int_mon_wgrads_by_oshape.push(int_mon_wgrads);
    side_mon_wgrads_by_oshape.push(side_mon_wgrads);
  } 

  (int_mon_wgrads_by_oshape, side_mon_wgrads_by_oshape)
}


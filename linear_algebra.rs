use common::{R, vec_with_len};
use dense_matrix::DenseMatrix;
use sparse_matrix::{SparseMatrix, Symmetric, StructurallySymmetric};

use std::libc::{c_double, c_ulong, c_int, c_uint, c_void, malloc, calloc, realloc, free};
use std::cast;
use std::libc;


pub type lapack_int = c_int; // Adjust according to whether LP64 or ILP64 libraries are being linked.
pub type mkl_int = c_int;    // Adjust according to whether LP64 or ILP64 libraries are being linked.

#[inline(never)]
pub fn init() {
  unsafe {
    init_allocator(cast::transmute(malloc), cast::transmute(calloc), cast::transmute(realloc), cast::transmute(free));
  }
}

#[inline(never)]
pub fn solve_sparse(sys: &SparseMatrix, rhs: &DenseMatrix) -> ~[R] {
  let n = sys.num_rows();

  unsafe {
    let (a, ia, ja) = sys.csr3_ptrs();  
    let mut sol = vec_with_len(n);
    let cpu_cores = num_cpus() as c_uint;

    let stat = match sys.matrix_type() {
      Symmetric => 
        mkl_solve_sparse_symmetric_as_ut_csr3(n as mkl_int, ia, ja, a,
                                              rhs.col_maj_data_ptr(), rhs.num_cols() as mkl_int,
                                              sol.as_mut_ptr(),
                                              cpu_cores),
      StructurallySymmetric =>
        mkl_solve_sparse_structurally_symmetric_csr3(n as mkl_int, ia, ja, a,
                                                     rhs.col_maj_data_ptr(), rhs.num_cols() as mkl_int,
                                                     sol.as_mut_ptr(),
                                                     cpu_cores),
      _ => 
        fail!("TODO: Support umfpack here on OS X."),
        //umf_solve_sparse_csr3(n as umf_int, ia, ja, a, rhs.col_maj_data_ptr(), sol.as_mut_ptr()),
    };

    if stat != 0 {
      fail!(format!("solve_sparse_symmetric_as_ut_csr3 failed with error {:d}", stat));
    }

    sol
  }
}

/* TODO: This isn't the preferred way to link anymore (too platform specific), so requires feature gate in wgfem.rs.
         I'm not sure how to specify the -L option otherwise though. */
#[link_args = "lib/linear_algebra.o -Llib/mkl -lmkl_intel_lp64 -lmkl_core -lmkl_intel_thread -lmkl_core -lmkl_intel_thread -lmkl_core -liomp5 -lpthread"] // -lumfpack
extern {

  pub fn init_allocator(malloc_fn: *c_void, calloc_fn: *c_void, realloc_fn: *c_void, free_fn: *c_void);

  pub fn alloc_doubles(num_doubles: c_ulong) -> *mut c_double;
  
  pub fn alloc_ints(num_ints: c_ulong) -> *mut lapack_int;

  pub fn free_doubles(mem: *mut c_double);
  
  pub fn free_ints(mem: *mut lapack_int);

  pub fn copy_matrix(from_data: *c_double, num_rows: c_ulong, num_cols: c_ulong, to_data: *mut c_double);
  
  pub fn copy_upper_triangle(from_data: *c_double, num_rows: c_ulong, num_cols: c_ulong, to_data: *mut c_double);


  /* Dense symmetric matrix system solver. */
  pub fn solve_symmetric_as_col_maj_with_ut_sys(a: *mut c_double,
                                                n: lapack_int,
                                                b: *mut c_double,
                                                nrhs: lapack_int,
                                                ipiv: *mut lapack_int) -> lapack_int;
  
  /* MKL sparse symmetric matrix system solver. */
  pub fn mkl_solve_sparse_symmetric_as_ut_csr3(n: mkl_int, ia: *mkl_int, ja: *mkl_int, a: *c_double,
                                               b: *c_double, nrhs: mkl_int,
                                               x: *mut c_double,
                                               num_cpu_cores: c_uint) -> mkl_int;

  /* MKL sparse structurally symmetric matrix system solver. */
  pub fn mkl_solve_sparse_structurally_symmetric_csr3(n: mkl_int, ia: *mkl_int, ja: *mkl_int, a: *c_double,
                                                      b: *c_double, nrhs: mkl_int,
                                                      x: *mut c_double,
                                                      num_cpu_cores: c_uint) -> mkl_int;
  
  /* UMFPACK general sparse matrix system solver. */
  // Works, commented out for now for convenience on OS X.
  //pub fn umf_solve_sparse_csr3(n: umf_int, ia: *umf_int, ja: *umf_int, a: *c_double, b: *c_double, x: *mut c_double) -> umf_int;
}

fn num_cpus() -> uint {
  unsafe {
    return rust_get_num_cpus();
  }

  extern {
    fn rust_get_num_cpus() -> libc::uintptr_t;
  }
}


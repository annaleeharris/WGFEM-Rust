use la;
use common::R;
use sparse_matrix::{SparseMatrix, Symmetric, StructurallySymmetric};
use dense_matrix::DenseMatrix;
use std::num::abs;

#[test]
fn test_do_la_init() {
  la::init(); // TODO: Do this somewhere else, where it's gauranteed to be run before other tests as part of each test setup.
}


fn approx_eq(v1: &[R], v2: &[R], tol: R) {
  if v1.len() != v2.len() || !v1.iter().zip(v2.iter()).all(|(&a,&b)| abs(a-b) <= tol) {
    fail!("Vectors not approximately equal: left value was {}, right was {}", v1.to_str(), v2.to_str());
  }
}

#[test]
fn test_sparse_symmetric_solve1() {
  //      1 0 0
  // A =  0 2 0
  //      0 0 3 
  // b = [3 2 1]^t
  // sol = [3 1 1/3]^t
  let mut A = SparseMatrix::new_with_capacities(3, 3, Symmetric);
  A.push(0,0, 1.);
  A.push(1,1, 2.);
  A.push(2,2, 3.);
  
  let b = DenseMatrix::from_rows(3,1, [~[3.],~[2.],~[1.]]);
  
  let sol = la::solve_sparse(&A, &b);

  approx_eq(sol, [3., 1., 1./3.], 1e-15);
}

#[test]
fn test_sparse_symmetric_solve2() {
  //      1 2 3
  // A =  2 2 0
  //      3 0 3 
  // b = [3 2 1]^t
  // sol = [0 1 1/3]^t
  let mut A = SparseMatrix::new_with_capacities(5, 3, Symmetric);
  A.push(0,0, 1.);
  A.push(0,1, 2.);
  A.push(0,2, 3.);
  //A.push(1,0, 2.);
  A.push(1,1, 2.);
  //A.push(2,0, 3.);
  A.push(2,2, 3.);
  
  let b = DenseMatrix::from_rows(3,1, [~[3.],~[2.],~[1.]]);

  let sol = la::solve_sparse(&A, &b);

  approx_eq(sol, [0., 1., 1./3.], 1e-15);
}

#[test]
#[should_fail]
fn test_sparse_symmetric_solve_bad_entry() {
  //      1 2 3
  // A =  2 2 0
  //      3 0 3 
  // b = [3 2 1]^t
  // sol = [0 1 1/3]^t
  let mut A = SparseMatrix::new_with_capacities(7, 3, Symmetric);
  A.push(0,0, 1.);
  A.push(0,1, 2.);
  A.push(0,2, 3.);
  A.push(1,0, 2.); // Should cause error: cannot include lower triangular entries in symmetric solve.
  A.push(1,1, 2.);
  A.push(2,0, 3.); // "
  A.push(2,2, 3.);
  
  let b = DenseMatrix::from_rows(3,1, [~[3.],~[2.],~[1.]]);

  la::solve_sparse(&A, &b);
}

#[test]
fn test_sparse_asymmetric_solve() {
  //      1 2 3
  // A =  2 1 0
  //      3 0 3 
  // b = [3 2 1]^t
  // sol = [1/3 4/3 0]^t
  let mut A = SparseMatrix::new_with_capacities(7, 3, StructurallySymmetric);
  A.push(0,0, 1.);
  A.push(0,1, 2.);
  A.push(0,2, 3.);
  A.push(1,0, 2.);
  A.push(1,1, 1.);
  A.push(2,0, 3.);
  A.push(2,2, 3.);
  
  let b = DenseMatrix::from_rows(3,1, [~[3.],~[2.],~[1.]]);

  let sol = la::solve_sparse(&A, &b);

  approx_eq(sol, [1./3., 4./3., 0.], 1e-15);
}


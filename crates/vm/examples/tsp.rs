// Copyright (C) 2026 Postquant Labs Incorporated
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Travelling Salesman Problem showcase using the XQVM interpreter.
//!
//! This example demonstrates a three-program XQVM pipeline for formulating,
//! verifying, and decoding a TSP as a QUBO (Quadratic Unconstrained Binary
//! Optimisation) problem.  All inter-program data (models, vectors, integers)
//! flows through calldata slots and output slots via the `INPUT`/`OUTPUT`
//! instructions.
//!
//! # Problem encoding
//!
//! For N cities, the QUBO uses N*N binary variables x[city*N + position].
//! The Hamiltonian is:
//!
//! H = H_dist + H_row + H_col
//!
//! Where:
//! - H_dist: sum d[ci][cj]*x[ci*N+p]*x[cj*N+(p+1)%N] for adjacent cities
//! - H_row:  penalty*(sum_p x[ci*N+p] - 1)^2 for each city ci
//! - H_col:  penalty*(sum_c x[c*N+p]  - 1)^2 for each position p
//!
//! # Pipeline
//!
//! 1. **Encoder** -- reads `N` and `distances` from input slots, builds the
//!    QUBO model, and writes it to output slot 0.
//! 2. **Verifier** -- reads the model, sample, and `N` from input slots;
//!    checks row/column one-hot constraints; writes energy and validity flag
//!    to output slots.
//! 3. **Decoder** -- reads the sample and `N` from input slots; extracts the
//!    ordered tour; writes it to output slot 0.

use aglais_xqvm_asm::assemble_source;
use aglais_xqvm_vm::model::{Domain, XqmxModel};
use aglais_xqvm_vm::value::RegVal;
use aglais_xqvm_vm::vm::Vm;

// ---------------------------------------------------------------------------
// Encoder assembly
// ---------------------------------------------------------------------------
// Input slots:  [0] = N (Int), [1] = distances (VecInt, N*N row-major flat)
// Output slots: [0] = BQMX model

const ENCODER_ASM: &str = "
PUSH 0
INPUT r0
PUSH 1
INPUT r1

LOAD r0
DUPL
MUL
STOW r2
LOAD r2
BQMX r4
LOAD r0
LOAD r0
RESIZE r4

PUSH 100
STOW r3

PUSH 0
LOAD r0
RANGE
  LVAL r5
  PUSH 0
  LOAD r0
  RANGE
    LVAL r6
    LOAD r5
    LOAD r6
    LT
    NOT
    JUMPI skip_dist
    LOAD r5
    LOAD r0
    MUL
    LOAD r6
    ADD
    VECGET r1
    STOW r7
    PUSH 0
    LOAD r0
    RANGE
      LVAL r8
      LOAD r8
      PUSH 1
      ADD
      LOAD r0
      MOD
      STOW r9
      LOAD r5
      LOAD r0
      MUL
      LOAD r8
      ADD
      LOAD r6
      LOAD r0
      MUL
      LOAD r9
      ADD
      LOAD r7
      ADDQUAD r4
      LOAD r6
      LOAD r0
      MUL
      LOAD r8
      ADD
      LOAD r5
      LOAD r0
      MUL
      LOAD r9
      ADD
      LOAD r7
      ADDQUAD r4
    NEXT
    skip_dist:
  NEXT
NEXT

PUSH 0
LOAD r0
RANGE
  LVAL r5
  LOAD r5
  LOAD r3
  ONEHOT r4
NEXT

PUSH 0
LOAD r0
RANGE
  LVAL r5
  PUSH 0
  LOAD r0
  RANGE
    LVAL r6
    LOAD r6
    LOAD r0
    MUL
    LOAD r5
    ADD
    LOAD r3
    NEG
    ADDLINE r4
  NEXT
  PUSH 0
  LOAD r0
  RANGE
    LVAL r6
    PUSH 0
    LOAD r0
    RANGE
      LVAL r7
      LOAD r6
      LOAD r7
      LT
      NOT
      JUMPI skip_col
      LOAD r6
      LOAD r0
      MUL
      LOAD r5
      ADD
      LOAD r7
      LOAD r0
      MUL
      LOAD r5
      ADD
      PUSH 2
      LOAD r3
      MUL
      ADDQUAD r4
      skip_col:
    NEXT
  NEXT
NEXT

PUSH 0
OUTPUT r4
HALT
";

// ---------------------------------------------------------------------------
// Verifier assembly
// ---------------------------------------------------------------------------
// Input slots:  [0] = model (Model), [1] = sample (Model NxN), [2] = N (Int)
// Output slots: [0] = energy (Int), [1] = is_valid (Int, 1 = valid)

const VERIFIER_ASM: &str = "
PUSH 0
INPUT r0
PUSH 1
INPUT r1
PUSH 2
INPUT r3

PUSH 1
STOW r4

PUSH 0
LOAD r3
RANGE
  LVAL r10
  LOAD r10
  ROWSUM r1
  PUSH 1
  EQ
  LOAD r4
  AND
  STOW r4
NEXT

PUSH 0
LOAD r3
RANGE
  LVAL r10
  LOAD r10
  COLSUM r1
  PUSH 1
  EQ
  LOAD r4
  AND
  STOW r4
NEXT

ENERGY r0 r1
STOW r5
PUSH 0
OUTPUT r5
PUSH 1
OUTPUT r4
HALT
";

// ---------------------------------------------------------------------------
// Decoder assembly
// ---------------------------------------------------------------------------
// Input slots:  [0] = sample (Model NxN), [1] = N (Int)
// Output slots: [0] = tour (VecInt, tour[p] = city at position p)

const DECODER_ASM: &str = "
PUSH 0
INPUT r0
PUSH 1
INPUT r1

VECI r2
PUSH 0
LOAD r1
RANGE
  LVAL r10
  LOAD r10
  PUSH 1
  COLFIND r0
  VECPUSH r2
NEXT

PUSH 0
OUTPUT r2
HALT
";

// ---------------------------------------------------------------------------
// Example driver
// ---------------------------------------------------------------------------

fn main() {
    // 4-city ring: distance i<->j = min(|i-j|, 4-|i-j|).
    // Row-major flat distance matrix (symmetric, zeros on diagonal).
    let n: usize = 4;
    #[rustfmt::skip]
    let distances: Vec<i64> = vec![
        0, 1, 2, 3,
        1, 0, 1, 2,
        2, 1, 0, 1,
        3, 2, 1, 0,
    ];

    // -- Step 1: encode the TSP as a QUBO model ---------------------------------

    let encoder_bc = assemble_source(ENCODER_ASM).expect("encoder assembly failed");

    let mut vm = Vm::new();
    vm.set_calldata(vec![RegVal::Int(n as i64), RegVal::VecInt(distances)]);
    vm.set_output_slots(1);
    vm.run(&encoder_bc).expect("encoder run failed");

    let qubo = vm.outputs()[0].clone();

    if let RegVal::Model(ref m) = qubo {
        println!(
            "QUBO: {} variables, {} linear terms, {} quadratic terms",
            m.size,
            m.linear_len(),
            m.quadratic_len()
        );
    }

    // -- Step 2: build the identity sample (city i visits position i) -----------
    //
    // Samples are stored as XqmxModel with linear[city*N + position] = 1.
    // This lets ROWSUM, COLSUM, COLFIND, and ENERGY operate on the sample
    // using the standard model grid instructions.

    let mut sample = XqmxModel::new(Domain::Binary, n * n);
    sample.rows = n;
    sample.cols = n;
    for i in 0..n {
        sample.set_linear(i * n + i, 1);
    }
    let sample_val = RegVal::Model(sample);

    // -- Step 3: verify the sample ----------------------------------------------

    let verifier_bc = assemble_source(VERIFIER_ASM).expect("verifier assembly failed");

    let mut vm = Vm::new();
    vm.set_calldata(vec![qubo, sample_val.clone(), RegVal::Int(n as i64)]);
    vm.set_output_slots(2);
    vm.run(&verifier_bc).expect("verifier run failed");

    let energy = match vm.outputs()[0] {
        RegVal::Int(v) => v,
        ref other => panic!("expected Int for energy, got {other:?}"),
    };
    let is_valid = match vm.outputs()[1] {
        RegVal::Int(v) => v != 0,
        ref other => panic!("expected Int for valid flag, got {other:?}"),
    };

    // -- Step 4: decode the sample into an ordered tour -------------------------

    let decoder_bc = assemble_source(DECODER_ASM).expect("decoder assembly failed");

    let mut vm = Vm::new();
    vm.set_calldata(vec![sample_val, RegVal::Int(n as i64)]);
    vm.set_output_slots(1);
    vm.run(&decoder_bc).expect("decoder run failed");

    let tour = match vm.outputs()[0] {
        RegVal::VecInt(ref v) => v.clone(),
        ref other => panic!("expected VecInt for tour, got {other:?}"),
    };

    // -- Print results ----------------------------------------------------------

    println!("TSP ({n} cities, ring distances)");
    println!("Tour:   {tour:?}");
    println!("Energy: {energy}");
    println!("Valid:  {is_valid}");

    // Sanity: identity tour is valid and has the expected energy.
    // H_dist = d(0,1)+d(1,2)+d(2,3)+d(3,0) = 1+1+1+3 = 6
    // H_one_hot (linear) = 4 cities * 2 constraints * (-100) = -800
    // H_one_hot (quadratic) = 0 (one variable selected per row and column)
    // Total = 6 - 800 = -794
    assert!(
        is_valid,
        "identity tour must satisfy all one-hot constraints"
    );
    assert_eq!(energy, -794, "energy mismatch for identity tour");
    assert_eq!(
        tour,
        vec![0, 1, 2, 3],
        "identity tour should decode to [0,1,2,3]"
    );
}

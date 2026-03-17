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

//! Integration tests for the XQVM bytecode interpreter.

use aglais_xqvm_bytecode::builder::InstructionBuilder;
use aglais_xqvm_bytecode::types::Register;
use aglais_xqvm_vm::error::Error;
use aglais_xqvm_vm::model::Domain;
use aglais_xqvm_vm::value::RegVal;
use aglais_xqvm_vm::vm::Vm;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build bytecode and run it on a fresh VM; return the VM.
fn run(build: impl FnOnce(&mut InstructionBuilder)) -> Vm {
    let mut b = InstructionBuilder::new();
    build(&mut b);
    let bytecode = b.build().expect("builder build");
    let mut vm = Vm::new();
    vm.run(&bytecode).expect("vm run");
    vm
}

/// Build bytecode and run it on a fresh VM; expect an error.
fn run_err(build: impl FnOnce(&mut InstructionBuilder)) -> Error {
    let mut b = InstructionBuilder::new();
    build(&mut b);
    let bytecode = b.build().expect("builder build");
    let mut vm = Vm::new();
    vm.run(&bytecode).expect_err("expected error")
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

#[test]
fn add_two_numbers() {
    let vm = run(|b| {
        b.push(3).push(4).add().halt();
    });
    assert_eq!(vm.stack(), &[7]);
}

#[test]
fn sub_two_numbers() {
    let vm = run(|b| {
        b.push(10).push(3).sub().halt();
    });
    assert_eq!(vm.stack(), &[7]);
}

#[test]
fn mul_two_numbers() {
    let vm = run(|b| {
        b.push(6).push(7).mul().halt();
    });
    assert_eq!(vm.stack(), &[42]);
}

#[test]
fn div_two_numbers() {
    let vm = run(|b| {
        b.push(21).push(3).div().halt();
    });
    assert_eq!(vm.stack(), &[7]);
}

#[test]
fn div_truncates_towards_zero() {
    let vm = run(|b| {
        b.push(7).push(2).div().halt();
    });
    assert_eq!(vm.stack(), &[3]);
}

#[test]
fn modulo_basic() {
    let vm = run(|b| {
        b.push(17).push(5).modulo().halt();
    });
    assert_eq!(vm.stack(), &[2]);
}

#[test]
fn neg_positive() {
    let vm = run(|b| {
        b.push(42).neg().halt();
    });
    assert_eq!(vm.stack(), &[-42]);
}

#[test]
fn neg_negative() {
    let vm = run(|b| {
        b.push(-7).neg().halt();
    });
    assert_eq!(vm.stack(), &[7]);
}

#[test]
fn wrapping_add_overflow() {
    let vm = run(|b| {
        b.push(i64::MAX).push(1).add().halt();
    });
    assert_eq!(vm.stack(), &[i64::MIN]);
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

#[test]
fn eq_equal() {
    let vm = run(|b| {
        b.push(5).push(5).eq().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn eq_not_equal() {
    let vm = run(|b| {
        b.push(5).push(6).eq().halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn lt_true() {
    let vm = run(|b| {
        b.push(3).push(5).lt().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn lt_false() {
    let vm = run(|b| {
        b.push(5).push(3).lt().halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn gt_true() {
    let vm = run(|b| {
        b.push(5).push(3).gt().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn lte_equal() {
    let vm = run(|b| {
        b.push(5).push(5).lte().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn gte_greater() {
    let vm = run(|b| {
        b.push(6).push(5).gte().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

// ---------------------------------------------------------------------------
// Logical and bitwise operators
// ---------------------------------------------------------------------------

#[test]
fn not_zero() {
    let vm = run(|b| {
        b.push(0).not().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn not_nonzero() {
    let vm = run(|b| {
        b.push(99).not().halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn and_both_nonzero() {
    let vm = run(|b| {
        b.push(1).push(1).and().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn and_one_zero() {
    let vm = run(|b| {
        b.push(1).push(0).and().halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn or_one_nonzero() {
    let vm = run(|b| {
        b.push(0).push(1).or().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn xor_different() {
    let vm = run(|b| {
        b.push(1).push(0).xor().halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

#[test]
fn xor_same() {
    let vm = run(|b| {
        b.push(1).push(1).xor().halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn band_basic() {
    let vm = run(|b| {
        b.push(0b1100).push(0b1010).b_and().halt();
    });
    assert_eq!(vm.stack(), &[0b1000]);
}

#[test]
fn bor_basic() {
    let vm = run(|b| {
        b.push(0b1100).push(0b1010).b_or().halt();
    });
    assert_eq!(vm.stack(), &[0b1110]);
}

#[test]
fn bxor_basic() {
    let vm = run(|b| {
        b.push(0b1100).push(0b1010).b_xor().halt();
    });
    assert_eq!(vm.stack(), &[0b0110]);
}

#[test]
fn bnot_basic() {
    let vm = run(|b| {
        b.push(0i64).b_not().halt();
    });
    assert_eq!(vm.stack(), &[-1i64]);
}

#[test]
fn shl_basic() {
    let vm = run(|b| {
        b.push(1).push(4).shl().halt();
    });
    assert_eq!(vm.stack(), &[16]);
}

#[test]
fn shr_basic() {
    // Logical right shift: sign bit is not replicated.
    let vm = run(|b| {
        b.push(-1i64).push(1).shr().halt();
    });
    assert_eq!(vm.stack(), &[i64::MAX]);
}

// ---------------------------------------------------------------------------
// Stack operations
// ---------------------------------------------------------------------------

#[test]
fn push_pop() {
    let vm = run(|b| {
        b.push(42).pop().halt();
    });
    assert!(vm.stack().is_empty());
}

#[test]
fn dupl_duplicates_top() {
    let vm = run(|b| {
        b.push(7).dupl().halt();
    });
    assert_eq!(vm.stack(), &[7, 7]);
}

#[test]
fn swap_swaps_top_two() {
    let vm = run(|b| {
        b.push(1).push(2).swap().halt();
    });
    assert_eq!(vm.stack(), &[2, 1]);
}

// ---------------------------------------------------------------------------
// Register load/store
// ---------------------------------------------------------------------------

#[test]
fn stow_and_load() {
    let vm = run(|b| {
        b.push(99).stow(Register(0)).load(Register(0)).halt();
    });
    assert_eq!(vm.stack(), &[99]);
}

#[test]
fn default_register_is_zero() {
    let vm = run(|b| {
        b.load(Register(5)).halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

// ---------------------------------------------------------------------------
// Unconditional and conditional jumps
// ---------------------------------------------------------------------------

#[test]
fn unconditional_jump_skips_code() {
    // PUSH 1 / JUMP over PUSH 99 / PUSH 2 / HALT
    let mut b = InstructionBuilder::new();
    let skip = b.label();
    b.push(1).jump(skip).push(99).place(skip).push(2).halt();
    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    // stack should be [1, 2] -- 99 was skipped
    assert_eq!(vm.stack(), &[1, 2]);
}

#[test]
fn conditional_jump_taken() {
    let mut b = InstructionBuilder::new();
    let done = b.label();
    b.push(1).jump_if(done).push(99).place(done).push(2).halt();
    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[2]);
}

#[test]
fn conditional_jump_not_taken() {
    let mut b = InstructionBuilder::new();
    let done = b.label();
    b.push(0).jump_if(done).push(99).place(done).push(2).halt();
    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[99, 2]);
}

// ---------------------------------------------------------------------------
// RANGE loop counting from 0 to N, accumulating a sum
// ---------------------------------------------------------------------------

#[test]
fn range_loop_sum_0_to_5() {
    // RANGE loop: sum = 0+1+2+3+4 = 10 (range [0, 5)).
    // The loop body executes once per value in [0, 5).
    let mut b = InstructionBuilder::new();

    b.push(0).stow(Register(0)); // r0 = 0 (accumulator)
    b.push(0).push(5).range(); // RANGE [0, 5)
    b.l_val(Register(1)); // r1 = current loop value
    b.load(Register(1))
        .load(Register(0))
        .add()
        .stow(Register(0)); // r0 += r1
    b.next();
    b.load(Register(0)).halt();

    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[10]);
}

#[test]
fn range_loop_do_while_semantics() {
    // RANGE with count=0: body still runs once (do-while semantics).
    // RANGE [0, 0): current=0, end=0. Body runs once, NEXT: 1 < 0 is false, exits.
    let mut b = InstructionBuilder::new();
    b.push(0).push(0).range();
    b.push(99); // body: always runs once
    b.next();
    b.halt();
    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[99]);
}

// ---------------------------------------------------------------------------
// ITER loop over a vec
// ---------------------------------------------------------------------------

#[test]
fn iter_loop_over_vec_int() {
    // Build [10, 20, 30] in r0, then ITER and sum elements into r1.
    let mut b = InstructionBuilder::new();

    b.vec_i(Register(0)); // r0 = []
    b.push(10).vec_push(Register(0));
    b.push(20).vec_push(Register(0));
    b.push(30).vec_push(Register(0));

    b.push(0).stow(Register(1)); // r1 = 0 (accumulator)
    b.iter(Register(0)); // ITER r0
    b.l_val(Register(2)); // r2 = current element
    b.load(Register(2))
        .load(Register(1))
        .add()
        .stow(Register(1));
    b.next();
    b.load(Register(1)).halt();

    let bytecode = b.build().unwrap();
    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[60]);
}

// ---------------------------------------------------------------------------
// VEC/VECI operations
// ---------------------------------------------------------------------------

#[test]
fn vec_push_get_set_len() {
    // Stack order for VECSET: pop value first, then index.
    // So to set vec[0]=999, push idx=0 first, then val=999.
    let vm = run(|b| {
        b.vec_i(Register(0));
        b.push(100).vec_push(Register(0));
        b.push(200).vec_push(Register(0));
        b.push(300).vec_push(Register(0));
        // len should be 3
        b.vec_len(Register(0));
        // get index 1 -> 200
        b.push(1).vec_get(Register(0));
        // set index 0 to 999: push idx first, val second
        b.push(0).push(999).vec_set(Register(0));
        // get index 0 -> 999
        b.push(0).vec_get(Register(0));
        b.halt();
    });
    // stack should be [3, 200, 999]
    assert_eq!(vm.stack(), &[3, 200, 999]);
}

// ---------------------------------------------------------------------------
// XQMX model allocation (BQMX), setline, getline, setquad, getquad
//
// Stack convention for model instructions (pops from top):
//   SETLINE:  pop val (top), pop i  -> set linear[i] = val
//   ADDLINE:  pop delta (top), pop i -> add delta to linear[i]
//   GETLINE:  pop i -> push linear[i]
//   SETQUAD:  pop val (top), pop j, pop i -> set quad[i,j] = val
//   ADDQUAD:  pop delta (top), pop j, pop i -> add delta to quad[i,j]
//   GETQUAD:  pop j (top), pop i -> push quad[i,j]
// ---------------------------------------------------------------------------

#[test]
fn bqmx_setline_getline() {
    // To set linear[2]=7: push i=2 first, then val=7 on top.
    let vm = run(|b| {
        b.push(4).bqmx(Register(0)); // r0 = QUBO(4)
        // setline: i=2, val=7 -- push i first, val on top
        b.push(2).push(7).set_line(Register(0));
        // getline: i=2 -> 7
        b.push(2).get_line(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[7]);
}

#[test]
fn bqmx_addline() {
    // set linear[0]=5, then add 3: linear[0] should be 8.
    // SETLINE: push i=0, push val=5. ADDLINE: push i=0, push delta=3.
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(0).push(5).set_line(Register(0));
        b.push(0).push(3).add_line(Register(0));
        b.push(0).get_line(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[8]);
}

#[test]
fn bqmx_setquad_getquad() {
    // To set quad[1,2]=-1: push i=1, j=2, val=-1 (val on top).
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        // setquad: i=1, j=2, val=-1 -- push i, j, val
        b.push(1).push(2).push(-1).set_quad(Register(0));
        // getquad: pop j=2 (top), pop i=1 -> push quad[1,2]=-1
        b.push(2).push(1).get_quad(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[-1]);
}

#[test]
fn bqmx_addquad() {
    // set quad[0,1]=3, add 2: quad[0,1] should be 5.
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(0).push(1).push(3).set_quad(Register(0));
        b.push(0).push(1).push(2).add_quad(Register(0));
        b.push(1).push(0).get_quad(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[5]);
}

#[test]
fn getline_absent_returns_zero() {
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(3).get_line(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

// ---------------------------------------------------------------------------
// ENERGY computation
// ---------------------------------------------------------------------------

#[test]
fn energy_simple_qubo_all_zero_sample() {
    // Model: H(x) = -x0 + 2*x0*x1
    // Sample: [0, 0] (BSMX default) -> H = 0
    let vm = run(|b| {
        b.push(2).bqmx(Register(0));
        // set linear[0] = -1: push i=0, val=-1
        b.push(0).push(-1).set_line(Register(0));
        // set quad[0,1] = 2: push i=0, j=1, val=2
        b.push(0).push(1).push(2).set_quad(Register(0));
        // BSMX creates sample initialized to [0, 0]
        b.push(2).bsmx(Register(1));
        b.energy(Register(0), Register(1));
        b.halt();
    });
    assert_eq!(vm.stack(), &[0]);
}

#[test]
fn energy_spin_ising_all_minus_one() {
    // Model: H(s) = s0*s1, sample = [-1, -1] -> H = (-1)*(-1) = 1
    let vm = run(|b| {
        b.push(2).sqmx(Register(0));
        // set quad[0,1] = 1: push i=0, j=1, val=1
        b.push(0).push(1).push(1).set_quad(Register(0));
        // SSMX creates sample initialized to [-1, -1]
        b.push(2).ssmx(Register(1));
        b.energy(Register(0), Register(1));
        b.halt();
    });
    assert_eq!(vm.stack(), &[1]);
}

// ---------------------------------------------------------------------------
// INPUT/OUTPUT
// ---------------------------------------------------------------------------

#[test]
fn input_output_roundtrip() {
    let mut b = InstructionBuilder::new();
    // Input slot 0 into r0, output r0 to slot 0.
    b.push(0).input(Register(0));
    b.push(0).output(Register(0));
    b.halt();
    let bytecode = b.build().unwrap();

    let mut vm = Vm::new();
    vm.set_calldata(vec![RegVal::Int(42)]).set_output_slots(4);
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.outputs()[0], RegVal::Int(42));
}

// ---------------------------------------------------------------------------
// IDXGRID and IDXTRIU
// ---------------------------------------------------------------------------

#[test]
fn idx_grid_basic() {
    // IDXGRID pops cols, col, row: row=1, col=2, cols=4 -> 1*4+2 = 6
    // Push row first (bottom), then col, then cols (top)
    let vm = run(|b| {
        b.push(1).push(2).push(4).idx_grid().halt();
    });
    assert_eq!(vm.stack(), &[6]);
}

#[test]
fn idx_triu_basic() {
    // IDXTRIU pops j (top), then i: i=1, j=3 -> 3*2/2 + 1 = 4
    // Push i first, j second (j on top)
    let vm = run(|b| {
        b.push(1).push(3).idx_triu().halt();
    });
    assert_eq!(vm.stack(), &[4]);
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn stack_underflow_on_pop() {
    let err = run_err(|b| {
        b.pop().halt();
    });
    assert!(matches!(err, Error::StackUnderflow { .. }));
}

#[test]
fn stack_underflow_on_add() {
    let err = run_err(|b| {
        b.push(1).add().halt();
    });
    assert!(matches!(err, Error::StackUnderflow { .. }));
}

#[test]
fn division_by_zero() {
    let err = run_err(|b| {
        b.push(5).push(0).div().halt();
    });
    assert!(matches!(err, Error::DivisionByZero { .. }));
}

#[test]
fn modulo_by_zero() {
    let err = run_err(|b| {
        b.push(5).push(0).modulo().halt();
    });
    assert!(matches!(err, Error::DivisionByZero { .. }));
}

#[test]
fn step_limit_exceeded() {
    // Infinite loop: PUSH 1 / JUMPI back to start.
    let mut b = InstructionBuilder::new();
    let top = b.label();
    b.place(top).push(1).jump_if(top).halt();
    let bytecode = b.build().unwrap();

    let mut vm = Vm::new();
    vm.set_step_limit(100);
    let err = vm.run(&bytecode).expect_err("expected step limit error");
    assert!(matches!(err, Error::StepLimitExceeded { .. }));
}

#[test]
fn invalid_shift_negative() {
    let err = run_err(|b| {
        b.push(1).push(-1).shl().halt();
    });
    assert!(matches!(err, Error::InvalidShift { .. }));
}

#[test]
fn invalid_shift_too_large() {
    let err = run_err(|b| {
        b.push(1).push(64).shl().halt();
    });
    assert!(matches!(err, Error::InvalidShift { .. }));
}

#[test]
fn register_type_error_load_on_model() {
    let err = run_err(|b| {
        b.push(4).bqmx(Register(0));
        b.load(Register(0)); // r0 is a model, not int
        b.halt();
    });
    assert!(matches!(err, Error::RegisterType { .. }));
}

#[test]
fn no_active_loop_next() {
    let err = run_err(|b| {
        b.next(); // raw NEXT with no loop
        b.halt();
    });
    assert!(matches!(err, Error::NoActiveLoop { .. }));
}

#[test]
fn index_out_of_bounds_vec_get() {
    let err = run_err(|b| {
        b.vec_i(Register(0));
        b.push(10).vec_push(Register(0));
        b.push(5).vec_get(Register(0)); // index 5 out of bounds
        b.halt();
    });
    assert!(matches!(err, Error::IndexOutOfBounds { .. }));
}

// ---------------------------------------------------------------------------
// ONEHOT / EXCLUDE / IMPLIES constraints
//
// Stack convention (pops from top):
//   ONEHOT:  pop penalty (top), pop row -> apply constraint to grid row
//   EXCLUDE: pop penalty (top), pop j, pop i -> penalise x_i * x_j = 1
//   IMPLIES: pop penalty (top), pop j, pop i -> penalise x_i=1, x_j=0
//   RESIZE:  pop cols (top), pop rows -> set grid dimensions
// ---------------------------------------------------------------------------

#[test]
fn one_hot_constraint_adds_coefficients() {
    // Grid: 1 row x 3 cols. ONEHOT row 0 with penalty 1.
    // To call ONEHOT with row=0, penalty=1: push row=0 first, penalty=1 on top.
    let vm = run(|b| {
        b.push(3).bqmx(Register(0));
        // resize: push rows=1 first, cols=3 on top
        b.push(1).push(3).resize(Register(0));
        // onehot: push row=0 first, penalty=1 on top
        b.push(0).push(1).one_hot(Register(0));
        b.halt();
    });
    let reg = vm.register(0);
    if let RegVal::Model(m) = reg {
        assert_eq!(m.get_linear(0), -1);
        assert_eq!(m.get_linear(1), -1);
        assert_eq!(m.get_linear(2), -1);
        assert_eq!(m.get_quad(0, 1), 2);
        assert_eq!(m.get_quad(0, 2), 2);
        assert_eq!(m.get_quad(1, 2), 2);
    } else {
        panic!("expected model register");
    }
}

#[test]
fn exclude_constraint_adds_coupling() {
    // exclude: i=1, j=2, penalty=5 -- push i=1, j=2, penalty=5 (penalty on top)
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(1).push(2).push(5).exclude(Register(0));
        b.halt();
    });
    let reg = vm.register(0);
    if let RegVal::Model(m) = reg {
        assert_eq!(m.get_quad(1, 2), 5);
    } else {
        panic!("expected model register");
    }
}

#[test]
fn implies_constraint_adds_linear_and_coupling() {
    // implies: i=0, j=1, penalty=3 -> linear[0] += 3; quad[0,1] += -3
    // push i=0, j=1, penalty=3 (penalty on top)
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(0).push(1).push(3).implies(Register(0));
        b.halt();
    });
    let reg = vm.register(0);
    if let RegVal::Model(m) = reg {
        assert_eq!(m.get_linear(0), 3);
        assert_eq!(m.get_quad(0, 1), -3);
    } else {
        panic!("expected model register");
    }
}

// ---------------------------------------------------------------------------
// Miscellaneous
// ---------------------------------------------------------------------------

#[test]
fn nop_does_nothing() {
    let vm = run(|b| {
        b.push(42).nop().nop().halt();
    });
    assert_eq!(vm.stack(), &[42]);
}

#[test]
fn halt_at_end_of_bytecode() {
    // No explicit HALT -- stream runs out, which is also fine.
    let vm = run(|b| {
        b.push(1).push(2).add();
        // no halt -- just let stream end
    });
    assert_eq!(vm.stack(), &[3]);
}

#[test]
fn reset_clears_state() {
    let mut b = InstructionBuilder::new();
    b.push(99).halt();
    let bytecode = b.build().unwrap();

    let mut vm = Vm::new();
    vm.run(&bytecode).unwrap();
    assert_eq!(vm.stack(), &[99]);
    vm.reset();
    assert!(vm.stack().is_empty());
}

#[test]
fn xqmx_discrete_model() {
    // XQMX: pops k (top) then size. push size=2, k=3.
    let vm = run(|b| {
        b.push(2).push(3).xqmx(Register(0)); // size=2, k=3
        b.halt();
    });
    let reg = vm.register(0);
    if let RegVal::Model(m) = reg {
        assert_eq!(m.domain, Domain::Discrete(3));
        assert_eq!(m.size, 2);
    } else {
        panic!("expected model register");
    }
}

#[test]
fn resize_sets_grid_dims() {
    // RESIZE: pops cols (top) then rows. push rows=3, cols=3.
    let vm = run(|b| {
        b.push(9).bqmx(Register(0));
        b.push(3).push(3).resize(Register(0));
        b.halt();
    });
    let reg = vm.register(0);
    if let RegVal::Model(m) = reg {
        assert_eq!(m.rows, 3);
        assert_eq!(m.cols, 3);
    } else {
        panic!("expected model register");
    }
}

#[test]
fn row_sum_and_col_sum() {
    // 2x2 grid: linear[0]=1, [1]=2, [2]=3, [3]=4.
    // rowsum(0) = linear[0]+linear[1] = 1+2 = 3
    // colsum(1) = linear[1]+linear[3] = 2+4 = 6
    //
    // SETLINE pops val (top) then i. Push i first, val second.
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        // resize: rows=2, cols=2
        b.push(2).push(2).resize(Register(0));
        // set linear[0]=1: push i=0, val=1
        b.push(0).push(1).set_line(Register(0));
        // set linear[1]=2: push i=1, val=2
        b.push(1).push(2).set_line(Register(0));
        // set linear[2]=3: push i=2, val=3
        b.push(2).push(3).set_line(Register(0));
        // set linear[3]=4: push i=3, val=4
        b.push(3).push(4).set_line(Register(0));
        b.push(0).row_sum(Register(0));
        b.push(1).col_sum(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[3, 6]);
}

#[test]
fn row_find_and_col_find() {
    // 2x2 grid: [0]=10, [1]=20, [2]=30, [3]=10.
    // ROWFIND: pops value (top) then row -> push first col where match, or -1
    // rowfind(row=0, value=20) -> col=1
    // COLFIND: pops value (top) then col -> push first row where match, or -1
    // colfind(col=1, value=10) -> row=1
    let vm = run(|b| {
        b.push(4).bqmx(Register(0));
        b.push(2).push(2).resize(Register(0));
        b.push(0).push(10).set_line(Register(0));
        b.push(1).push(20).set_line(Register(0));
        b.push(2).push(30).set_line(Register(0));
        b.push(3).push(10).set_line(Register(0));
        // rowfind: push row=0, value=20 on top
        b.push(0).push(20).row_find(Register(0));
        // colfind: push col=1, value=10 on top
        b.push(1).push(10).col_find(Register(0));
        b.halt();
    });
    assert_eq!(vm.stack(), &[1, 1]);
}

#![allow(dead_code)]

mod externals;

use crate::{Instruction, Result, RuntimeError, RuntimeErrorTy, RuntimeValue, Vm};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use once_cell::sync::OnceCell;
use std::{marker::PhantomData, mem};

/// Call a win64 function in assembly, requires the user to setup parameters and
/// replaces the Vm pointer into rcx  
/// Automatically handles errors, non-zero returns will be immediately returned from the entire function
macro_rules! call {
    ($asm:ident, $func:expr) => {{
        let skip = $asm.new_dynamic_label();
        dynasm!($asm
            ; mov rax, QWORD $func as _ // Move the function pointer into rax
            ; call rax                  // Execute the function at rax
            ; cmp rax, 0x00             // Load the value of rax into the comparison reg
            ; jz =>skip                 // If rax is zero, skip the return
            ; add rsp, 0x28             // Deallocate the memory allocated for the Vm ptr
            ; ret                       // Return with the value at rax
            ; =>skip
            ; mov rcx, [rsp + 0x30]     // Move the Vm pointer back into rax
        );
    }};
}

/// Get the offset of a type field
macro_rules! offset_of {
    ($instance:expr, $type:path, $field:tt) => {{
        let reference: &$type = &$instance;
        let address = reference as *const _ as usize;
        let $type {
            $field: field_ref, ..
        } = reference;
        let field_pointer = field_ref as *const _ as usize;

        field_pointer - address
    }};
}

#[derive(Debug, Clone)]
struct VmOffsets {
    pub registers: usize,
    pub return_stack: usize,
    pub current_func: usize,
    pub index: usize,
    pub finished_execution: usize,
    pub returning: usize,
    pub prev_op: usize,
    pub prev_comp: usize,
    pub gc: usize,
    pub options: usize,
    pub stdout: usize,
    pub start_time: usize,
}

impl VmOffsets {
    pub fn offsets(vm: &Vm) -> &'static VmOffsets {
        static INSTANCE: OnceCell<VmOffsets> = OnceCell::new();
        if INSTANCE.get().is_none() {
            INSTANCE
                .set(VmOffsets {
                    registers: offset_of!(vm, Vm, registers),
                    return_stack: offset_of!(vm, Vm, return_stack),
                    current_func: offset_of!(vm, Vm, current_func),
                    index: offset_of!(vm, Vm, index),
                    finished_execution: offset_of!(vm, Vm, finished_execution),
                    returning: offset_of!(vm, Vm, returning),
                    prev_op: offset_of!(vm, Vm, prev_op),
                    prev_comp: offset_of!(vm, Vm, prev_comp),
                    gc: offset_of!(vm, Vm, gc),
                    options: offset_of!(vm, Vm, options),
                    stdout: offset_of!(vm, Vm, stdout),
                    start_time: offset_of!(vm, Vm, start_time),
                })
                .unwrap();
        }
        INSTANCE.get().unwrap()
    }
}

dynasm!(asm
    ; .arch x64 // Asm currently written for x64
);

#[derive(Debug)]
pub struct Jit<'a> {
    code: dynasmrt::ExecutableBuffer,
    start: dynasmrt::AssemblyOffset,
    __value_lifetime: PhantomData<&'a RuntimeValue>,
    pub reference: Vec<Instruction>,
}

impl<'a> Jit<'a> {
    pub fn run(&self, vm: &'a mut Vm) -> Result<()> {
        let jit: extern "win64" fn(*mut Vm) -> usize =
            unsafe { mem::transmute(self.code.ptr(self.start)) };

        let res = jit(vm);
        if res == 0 {
            Ok(())
        } else {
            // Unsafe: Trusts that the given pointer is valid
            let err = unsafe { Box::from_raw(res as *mut RuntimeError) };
            Err(*err)
        }
    }

    pub fn new(instructions: Vec<Instruction>) -> Result<Jit<'a>> {
        let start_time = std::time::Instant::now();
        info!("Jitting function, started at {:?}", start_time);

        let mut asm = dynasmrt::x64::Assembler::new().unwrap();
        let (mut front_jumps, mut back_jumps, mut inst_ptr) = (Vec::new(), Vec::new(), 0);

        let start = asm.offset();
        dynasm!(asm
            ; sub rsp, 0x28         // Allocate the space for the Vm ptr
            ; mov [rsp + 0x30], rcx // Move the Vm ptr into the allocation
        );

        for instruction in &instructions {
            match instruction {
                Instruction::Load(val, reg) => {
                    dynasm!(asm
                        ; mov rdx, QWORD val as *const RuntimeValue as _
                        ; mov r8, BYTE **reg as _
                        ;; call!(asm, externals::load)
                    );
                }
                Instruction::CompToReg(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::comp_to_reg)
                    );
                }
                Instruction::OpToReg(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::op_to_reg)
                    );
                }
                Instruction::Drop(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::drop)
                    );
                }
                Instruction::Move(input, output) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **input as _
                        ; mov rdx, BYTE **output as _
                        ;; call!(asm, externals::mov)
                    );
                }

                Instruction::Add(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::add)
                    );
                }
                Instruction::Sub(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::sub)
                    );
                }
                Instruction::Mult(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::mult)
                    );
                }
                Instruction::Div(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::div)
                    );
                }

                Instruction::Print(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::print)
                    );
                }

                // Jumps can be optimized on the asm level
                Instruction::Jump(index) => {
                    if index.is_negative() {
                        if let Some(pos) = back_jumps
                            .iter()
                            .position(|(_, ptr)| *ptr == inst_ptr + index)
                        {
                            let (jump_point, _) = back_jumps.remove(pos);

                            dynasm!(asm
                                ; mov rdx, *index
                                ;; call!(asm, externals::jump)
                                ; cmp rax, 0
                                ; jz =>jump_point
                            );
                        } else {
                            panic!("Failed to find JIT back jump");
                        }
                    } else {
                        let jump_point = asm.new_dynamic_label();
                        front_jumps.push((jump_point, inst_ptr, *index));

                        dynasm!(asm
                            ; mov rdx, *index
                            ;; call!(asm, externals::jump)
                            ; cmp rax, 0
                            ; jz =>jump_point
                        );
                    }
                }
                Instruction::JumpComp(index) => {
                    if index.is_negative() {
                        if let Some(pos) = back_jumps
                            .iter()
                            .position(|(_, ptr)| *ptr == inst_ptr + index)
                        {
                            let (jump_point, _) = back_jumps.remove(pos);

                            dynasm!(asm
                                ; mov rdx, *index
                                ;; call!(asm, externals::jump_comp)
                                ; cmp rax, 0
                                ; jz =>jump_point
                            );
                        } else {
                            panic!("Failed to find JIT back jump");
                        }
                    } else {
                        let jump_point = asm.new_dynamic_label();
                        front_jumps.push((jump_point, inst_ptr, *index));

                        dynasm!(asm
                            ; mov rdx, *index
                            ;; call!(asm, externals::jump_comp)
                            ; cmp rax, 0
                            ; jz =>jump_point
                        );
                    }
                }
                Instruction::JumpPoint(_) => {
                    if let Some(pos) = front_jumps
                        .iter()
                        .position(|(_, ptr, index)| *ptr + index == inst_ptr)
                    {
                        let (jump_point, _, _) = front_jumps.remove(pos);

                        dynasm!(asm
                            ; =>jump_point
                        );
                    } else {
                        let jump_point = asm.new_dynamic_label();
                        back_jumps.push((jump_point, inst_ptr));

                        dynasm!(asm
                            ; =>jump_point
                        );
                    }
                }

                Instruction::And(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::and)
                    );
                }
                Instruction::Or(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::or)
                    );
                }
                Instruction::Xor(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::xor)
                    );
                }
                Instruction::Not(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::not)
                    );
                }

                Instruction::Eq(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::eq)
                    );
                }
                Instruction::NotEq(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::not_eq)
                    );
                }
                Instruction::GreaterThan(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::greater_than)
                    );
                }
                Instruction::LessThan(left, right) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **left as _
                        ; mov r8, BYTE **right as _
                        ;; call!(asm, externals::less_than)
                    );
                }

                Instruction::Func(func) => {
                    dynasm!(asm
                        ; mov rdx, *func as _
                        ;; call!(asm, externals::func)
                    );
                }
                Instruction::Yield => call!(asm, externals::yield_generator),
                Instruction::Return => call!(asm, externals::ret),

                Instruction::Collect => call!(asm, externals::collect),
                Instruction::Halt => call!(asm, externals::halt),
                Instruction::NoOp => call!(asm, externals::no_op),
                Instruction::Illegal => call!(asm, externals::illegal),
            }

            inst_ptr += 1;
        }

        dynasm!(asm
            ; mov rax, 0
            ; add rsp, 0x28
            ; ret
        );

        if front_jumps.len() > 0 || back_jumps.len() > 0 {
            error!(
                "JIT has leftover loops: \nForward Jumps: {:?}\nBackward Jumps: {:?}",
                front_jumps, back_jumps
            );
            return Err(RuntimeError {
                ty: RuntimeErrorTy::JitError,
                message: "JIT miss-compilation, leftover loops".to_string(),
            });
        }

        let code = asm.finalize().unwrap(); // TODO: Handle Error

        info!("Finished jitting function at {:?}", start_time.elapsed());
        Ok(Self {
            code,
            start,
            __value_lifetime: PhantomData,
            reference: instructions,
        })
    }
}

#[test]
fn jit_test() {
    let instructions = vec![
        Instruction::Load(RuntimeValue::I32(10), 0.into()),
        Instruction::Load(RuntimeValue::I32(10), 1.into()),
        Instruction::Add(0.into(), 1.into()),
        Instruction::Add(0.into(), 1.into()),
        Instruction::Load(RuntimeValue::Str("Test\n"), 0.into()),
        Instruction::Print(0.into()),
        Instruction::Print(0.into()),
        Instruction::Jump(4),
        Instruction::Drop(0.into()),
        Instruction::Load(RuntimeValue::Str("Test Two\n"), 0.into()),
        Instruction::Print(0.into()),
        Instruction::JumpPoint(0),
    ];
    let jit = Jit::new(instructions).unwrap();

    let mut vm = Vm::new(
        &crate::OptionBuilder::new("./jit_test").build(),
        Box::new(std::io::stdout()),
    );

    jit.run(&mut vm).unwrap();
}

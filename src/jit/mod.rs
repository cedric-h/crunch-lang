#![allow(dead_code)]

mod externals;

use crate::{Instruction, Result, RuntimeError, RuntimeErrorTy, RuntimeValue, Vm};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use std::{marker::PhantomData, mem};

// TODO: Jumps don't work, everything needs testing and then integration

macro_rules! call {
    ($asm:ident, $func:expr) => {
        dynasm!($asm
            ; mov rax, QWORD $func as _
            ; call rax
            ; mov rcx, [rsp + 0x30]
        );
    };
}

dynasm!(asm
    ; .arch x64
);

#[derive(Debug)]
struct Jit<'a> {
    code: dynasmrt::ExecutableBuffer,
    start: dynasmrt::AssemblyOffset,
    __value_lifetime: PhantomData<&'a RuntimeValue>,
}

impl<'a> Jit<'a> {
    pub fn run(&self, vm: &mut Vm) -> std::result::Result<(), &'static str> {
        let f: extern "win64" fn(*mut Vm) -> u8 =
            unsafe { mem::transmute(self.code.ptr(self.start)) };

        let res = f(vm);
        if res == 0 {
            Ok(())
        } else if res == 1 {
            Err("An overflow occurred")
        } else if res == 2 {
            Err("IO error")
        } else {
            panic!("Unknown error code: {}", res);
        }
    }

    pub fn new(instructions: &'a [Instruction]) -> Result<Jit<'a>> {
        let mut asm = dynasmrt::x64::Assembler::new().unwrap();
        let mut front_jumps = Vec::new();
        let mut back_jumps = Vec::new();
        let mut inst_ptr = 0;

        let start = asm.offset();
        dynasm!(asm
            ; sub rsp, 0x28
            ; mov [rsp + 0x30], rcx // Move the *mut Vm into rcx
        );

        for instruction in instructions {
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
                Instruction::DropReg(reg) => {
                    dynasm!(asm
                        ; mov rdx, BYTE **reg as _
                        ;; call!(asm, externals::drop_reg)
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
                                ; =>jump_point
                            );
                        } else {
                            panic!("Failed to find JIT back jump");
                        }
                    } else {
                        let jump_point = asm.new_dynamic_label();
                        front_jumps.push((jump_point, inst_ptr, *index));

                        dynasm!(asm
                            ;; call!(asm, externals::jump)
                            ; cmp BYTE [rax], 0
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
                                ; =>jump_point
                            );
                        } else {
                            panic!("Failed to find JIT back jump");
                        }
                    } else {
                        let jump_point = asm.new_dynamic_label();
                        front_jumps.push((jump_point, inst_ptr, *index));

                        dynasm!(asm
                            ;; call!(asm, externals::jump_comp)
                            ; cmp BYTE [rax], 0
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
        Ok(Self {
            code,
            start,
            __value_lifetime: PhantomData,
        })
    }
}

#[test]
fn jit_test() {
    simple_logger::init().unwrap();
    color_backtrace::install();

    let instructions = vec![
        Instruction::Add(0.into(), 1.into()),
        Instruction::Add(0.into(), 1.into()),
        Instruction::Load(RuntimeValue::Str("Test"), 0.into()),
        Instruction::Print(0.into()),
        Instruction::JumpPoint(0),
        Instruction::Print(0.into()),
        Instruction::Jump(-2),
        Instruction::Jump(3),
        Instruction::DropReg(0.into()),
        Instruction::Load(RuntimeValue::Str("Test Two"), 0.into()),
        Instruction::Print(0.into()),
    ];
    let jit = Jit::new(&instructions).unwrap();

    let mut vm = Vm::new(
        Vec::new(),
        &crate::OptionBuilder::new("./jit_test").build(),
        Box::new(std::io::stdout()),
    );
    Instruction::Load(RuntimeValue::I32(10), 0.into())
        .execute(&mut vm)
        .unwrap();
    Instruction::Load(RuntimeValue::I32(10), 1.into())
        .execute(&mut vm)
        .unwrap();

    jit.run(&mut vm).unwrap();
}

// Native LLVM backend using llvm-sys
// "Direct access to LLVM's power"

#[cfg(feature = "llvm")]
use llvm_sys::prelude::*;
#[cfg(feature = "llvm")]
use llvm_sys::core::*;
#[cfg(feature = "llvm")]
use llvm_sys::target::*;
#[cfg(feature = "llvm")]
use llvm_sys::target_machine::*;
#[cfg(feature = "llvm")]
use llvm_sys::transforms::pass_manager_builder::*;

use crate::ast::{Program, Item, Function, Type, Stmt, Expr, BinOp, AssignTarget};
use crate::errors::{CompileError, Result};
use std::collections::HashMap;
use std::ffi::{CString, CStr};
use std::path::PathBuf;
use std::ptr;

/// Native LLVM code generator
pub struct LLVMNativeGenerator {
    #[cfg(feature = "llvm")]
    context: LLVMContextRef,
    #[cfg(feature = "llvm")]
    module: LLVMModuleRef,
    #[cfg(feature = "llvm")]
    builder: LLVMBuilderRef,
    #[cfg(feature = "llvm")]
    functions: HashMap<String, LLVMValueRef>,
    #[cfg(feature = "llvm")]
    variables: HashMap<String, LLVMValueRef>,
    module_name: String,
}

impl LLVMNativeGenerator {
    pub fn new(module_name: &str) -> Result<Self> {
        #[cfg(feature = "llvm")]
        unsafe {
            let context = LLVMContextCreate();
            let module_name_cstr = CString::new(module_name).unwrap();
            let module = LLVMModuleCreateWithNameInContext(module_name_cstr.as_ptr(), context);
            let builder = LLVMCreateBuilderInContext(context);
            
            Ok(Self {
                context,
                module,
                builder,
                functions: HashMap::new(),
                variables: HashMap::new(),
                module_name: module_name.to_string(),
            })
        }
        
        #[cfg(not(feature = "llvm"))]
        Ok(Self {
            module_name: module_name.to_string(),
        })
    }
    
    /// Compile a program to native code
    pub fn compile(&mut self, program: &Program) -> Result<()> {
        #[cfg(feature = "llvm")]
        unsafe {
            // Initialize target
            LLVM_InitializeNativeTarget();
            LLVM_InitializeNativeAsmPrinter();
            
            // Declare external functions
            self.declare_externals()?;
            
            // Generate functions
            for item in &program.items {
                match item {
                    Item::Function(func) => {
                        self.generate_function(func)?;
                    }
                    _ => {
                        // Skip other items for now
                    }
                }
            }
            
            // Verify module
            let error = ptr::null_mut();
            if LLVMVerifyModule(self.module, LLVMVerifierFailureAction::LLVMPrintMessageAction, error) != 0 {
                if !error.is_null() {
                    let error_str = CStr::from_ptr(*error).to_string_lossy();
                    LLVMDisposeMessage(*error);
                    return Err(CompileError::Generic(format!("Module verification failed: {}", error_str)));
                }
            }
            
            Ok(())
        }
        
        #[cfg(not(feature = "llvm"))]
        Err(CompileError::Generic("LLVM support not enabled. Rebuild with --features llvm".to_string()))
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn declare_externals(&mut self) -> Result<()> {
        // Declare printf
        let printf_type = LLVMFunctionType(
            LLVMInt32TypeInContext(self.context),
            [LLVMPointerType(LLVMInt8TypeInContext(self.context), 0)].as_ptr() as *mut _,
            1,
            1, // variadic
        );
        let printf_name = CString::new("printf").unwrap();
        let printf_func = LLVMAddFunction(self.module, printf_name.as_ptr(), printf_type);
        self.functions.insert("printf".to_string(), printf_func);
        
        Ok(())
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn generate_function(&mut self, func: &Function) -> Result<()> {
        // Create function type
        let return_type = self.type_to_llvm(&func.return_type);
        let param_types: Vec<LLVMTypeRef> = func.params.iter()
            .map(|p| self.type_to_llvm(&Some(p.ty.clone())))
            .collect();
        
        let func_type = LLVMFunctionType(
            return_type,
            param_types.as_ptr() as *mut _,
            param_types.len() as u32,
            0, // not variadic
        );
        
        // Create function
        let func_name = CString::new(func.name.as_str()).unwrap();
        let llvm_func = LLVMAddFunction(self.module, func_name.as_ptr(), func_type);
        self.functions.insert(func.name.clone(), llvm_func);
        
        // Create entry block
        let entry_name = CString::new("entry").unwrap();
        let entry_block = LLVMAppendBasicBlockInContext(self.context, llvm_func, entry_name.as_ptr());
        LLVMPositionBuilderAtEnd(self.builder, entry_block);
        
        // Clear variables for new function
        self.variables.clear();
        
        // Add parameters to variables
        for (i, param) in func.params.iter().enumerate() {
            let param_value = LLVMGetParam(llvm_func, i as u32);
            let param_name = CString::new(param.name.as_str()).unwrap();
            LLVMSetValueName2(param_value, param_name.as_ptr(), param.name.len());
            
            // Allocate space for parameter
            let alloca = LLVMBuildAlloca(self.builder, self.type_to_llvm(&Some(param.ty.clone())), param_name.as_ptr());
            LLVMBuildStore(self.builder, param_value, alloca);
            self.variables.insert(param.name.clone(), alloca);
        }
        
        // Generate function body
        for stmt in &func.body {
            self.generate_statement(stmt)?;
        }
        
        // Add default return if needed
        if func.return_type.is_none() && !func.body.iter().any(|s| matches!(s, Stmt::Return(_))) {
            LLVMBuildRetVoid(self.builder);
        }
        
        Ok(())
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn type_to_llvm(&self, ty: &Option<Type>) -> LLVMTypeRef {
        match ty {
            None => LLVMVoidTypeInContext(self.context),
            Some(Type::I32) => LLVMInt32TypeInContext(self.context),
            Some(Type::I64) => LLVMInt64TypeInContext(self.context),
            Some(Type::U32) => LLVMInt32TypeInContext(self.context),
            Some(Type::U64) => LLVMInt64TypeInContext(self.context),
            Some(Type::Bool) => LLVMInt1TypeInContext(self.context),
            Some(Type::String) => LLVMPointerType(LLVMInt8TypeInContext(self.context), 0),
            Some(Type::Unit) => LLVMVoidTypeInContext(self.context),
            Some(Type::Array(elem_ty, size)) => {
                if let crate::ast::ArraySize::Known(n) = size {
                    LLVMArrayType(self.type_to_llvm(&Some(elem_ty.as_ref().clone())), *n as u32)
                } else {
                    // Dynamic arrays as pointers
                    LLVMPointerType(self.type_to_llvm(&Some(elem_ty.as_ref().clone())), 0)
                }
            },
            _ => LLVMPointerType(LLVMInt8TypeInContext(self.context), 0), // Default to pointer
        }
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn generate_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.generate_expression(expr)?;
            }
            
            Stmt::Let { name, value, .. } => {
                let value_ref = self.generate_expression(value)?;
                let ty = self.infer_expr_type(value);
                let alloca = LLVMBuildAlloca(self.builder, ty, CString::new(name.as_str()).unwrap().as_ptr());
                LLVMBuildStore(self.builder, value_ref, alloca);
                self.variables.insert(name.clone(), alloca);
            }
            
            Stmt::Return(Some(expr)) => {
                let value = self.generate_expression(expr)?;
                LLVMBuildRet(self.builder, value);
            }
            
            Stmt::Return(None) => {
                LLVMBuildRetVoid(self.builder);
            }
            
            Stmt::If { condition, then_branch, else_branch, .. } => {
                let cond_val = self.generate_expression(condition)?;
                
                let func = LLVMGetBasicBlockParent(LLVMGetInsertBlock(self.builder));
                let then_bb = LLVMAppendBasicBlockInContext(self.context, func, b"then\0".as_ptr() as *const _);
                let else_bb = LLVMAppendBasicBlockInContext(self.context, func, b"else\0".as_ptr() as *const _);
                let end_bb = LLVMAppendBasicBlockInContext(self.context, func, b"endif\0".as_ptr() as *const _);
                
                if else_branch.is_some() {
                    LLVMBuildCondBr(self.builder, cond_val, then_bb, else_bb);
                } else {
                    LLVMBuildCondBr(self.builder, cond_val, then_bb, end_bb);
                }
                
                // Then branch
                LLVMPositionBuilderAtEnd(self.builder, then_bb);
                for stmt in then_branch {
                    self.generate_statement(stmt)?;
                }
                LLVMBuildBr(self.builder, end_bb);
                
                // Else branch
                if let Some(else_stmts) = else_branch {
                    LLVMPositionBuilderAtEnd(self.builder, else_bb);
                    for stmt in else_stmts {
                        self.generate_statement(stmt)?;
                    }
                    LLVMBuildBr(self.builder, end_bb);
                }
                
                // Continue at end
                LLVMPositionBuilderAtEnd(self.builder, end_bb);
            }
            
            _ => {
                // TODO: Implement other statements
            }
        }
        
        Ok(())
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn generate_expression(&mut self, expr: &Expr) -> Result<LLVMValueRef> {
        match expr {
            Expr::Integer(n) => {
                Ok(LLVMConstInt(LLVMInt64TypeInContext(self.context), *n as u64, 0))
            }
            
            Expr::Bool(b) => {
                Ok(LLVMConstInt(LLVMInt1TypeInContext(self.context), *b as u64, 0))
            }
            
            Expr::String(s) => {
                let str_val = CString::new(s.as_str()).unwrap();
                let global_str = LLVMBuildGlobalStringPtr(self.builder, str_val.as_ptr(), b"str\0".as_ptr() as *const _);
                Ok(global_str)
            }
            
            Expr::Ident(name) => {
                if let Some(&var) = self.variables.get(name) {
                    let load_name = CString::new(format!("{}.load", name)).unwrap();
                    Ok(LLVMBuildLoad2(self.builder, LLVMInt64TypeInContext(self.context), var, load_name.as_ptr()))
                } else {
                    Err(CompileError::Generic(format!("Undefined variable: {}", name)))
                }
            }
            
            Expr::Binary { left, op, right, .. } => {
                let left_val = self.generate_expression(left)?;
                let right_val = self.generate_expression(right)?;
                
                let result = match op {
                    BinOp::Add => LLVMBuildAdd(self.builder, left_val, right_val, b"add\0".as_ptr() as *const _),
                    BinOp::Sub => LLVMBuildSub(self.builder, left_val, right_val, b"sub\0".as_ptr() as *const _),
                    BinOp::Mul => LLVMBuildMul(self.builder, left_val, right_val, b"mul\0".as_ptr() as *const _),
                    BinOp::Div => LLVMBuildSDiv(self.builder, left_val, right_val, b"div\0".as_ptr() as *const _),
                    BinOp::Lt => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntSLT, left_val, right_val, b"lt\0".as_ptr() as *const _),
                    BinOp::Le => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntSLE, left_val, right_val, b"le\0".as_ptr() as *const _),
                    BinOp::Gt => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntSGT, left_val, right_val, b"gt\0".as_ptr() as *const _),
                    BinOp::Ge => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntSGE, left_val, right_val, b"ge\0".as_ptr() as *const _),
                    BinOp::Eq => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntEQ, left_val, right_val, b"eq\0".as_ptr() as *const _),
                    BinOp::Ne => LLVMBuildICmp(self.builder, LLVMIntPredicate::LLVMIntNE, left_val, right_val, b"ne\0".as_ptr() as *const _),
                    _ => return Err(CompileError::Generic("Unsupported binary operator".to_string())),
                };
                
                Ok(result)
            }
            
            Expr::Call { func, args, .. } => {
                if let Expr::Ident(func_name) = func.as_ref() {
                    match func_name.as_str() {
                        "print_int" => {
                            if args.len() == 1 {
                                let arg_val = self.generate_expression(&args[0])?;
                                let format_str = LLVMBuildGlobalStringPtr(self.builder, b"%lld\n\0".as_ptr() as *const _, b"fmt\0".as_ptr() as *const _);
                                let printf = *self.functions.get("printf").unwrap();
                                let call_args = vec![format_str, arg_val];
                                let call = LLVMBuildCall2(
                                    self.builder,
                                    LLVMGetElementType(LLVMTypeOf(printf)),
                                    printf,
                                    call_args.as_ptr() as *mut _,
                                    call_args.len() as u32,
                                    b"call\0".as_ptr() as *const _,
                                );
                                Ok(call)
                            } else {
                                Err(CompileError::Generic("print_int expects 1 argument".to_string()))
                            }
                        }
                        _ => {
                            // User-defined function
                            if let Some(&func_ref) = self.functions.get(func_name) {
                                let mut arg_vals = Vec::new();
                                for arg in args {
                                    arg_vals.push(self.generate_expression(arg)?);
                                }
                                let call = LLVMBuildCall2(
                                    self.builder,
                                    LLVMGetElementType(LLVMTypeOf(func_ref)),
                                    func_ref,
                                    arg_vals.as_ptr() as *mut _,
                                    arg_vals.len() as u32,
                                    b"call\0".as_ptr() as *const _,
                                );
                                Ok(call)
                            } else {
                                Err(CompileError::Generic(format!("Undefined function: {}", func_name)))
                            }
                        }
                    }
                } else {
                    Err(CompileError::Generic("Complex function calls not yet supported".to_string()))
                }
            }
            
            _ => {
                Err(CompileError::Generic("Expression type not yet implemented".to_string()))
            }
        }
    }
    
    #[cfg(feature = "llvm")]
    unsafe fn infer_expr_type(&self, expr: &Expr) -> LLVMTypeRef {
        match expr {
            Expr::Integer(_) => LLVMInt64TypeInContext(self.context),
            Expr::Bool(_) => LLVMInt1TypeInContext(self.context),
            Expr::String(_) => LLVMPointerType(LLVMInt8TypeInContext(self.context), 0),
            _ => LLVMInt64TypeInContext(self.context), // Default
        }
    }
    
    /// Write output to object file
    pub fn write_object_file(&self, output_path: &PathBuf) -> Result<()> {
        #[cfg(feature = "llvm")]
        unsafe {
            // Get target triple
            let target_triple = LLVMGetDefaultTargetTriple();
            
            // Get target
            let mut target = ptr::null_mut();
            let mut error = ptr::null_mut();
            if LLVMGetTargetFromTriple(target_triple, &mut target, &mut error) != 0 {
                if !error.is_null() {
                    let error_str = CStr::from_ptr(error).to_string_lossy();
                    LLVMDisposeMessage(error);
                    return Err(CompileError::Generic(format!("Failed to get target: {}", error_str)));
                }
            }
            
            // Create target machine
            let cpu = b"generic\0".as_ptr() as *const _;
            let features = b"\0".as_ptr() as *const _;
            let target_machine = LLVMCreateTargetMachine(
                target,
                target_triple,
                cpu,
                features,
                LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
                LLVMRelocMode::LLVMRelocDefault,
                LLVMCodeModel::LLVMCodeModelDefault,
            );
            
            // Write object file
            let output_cstr = CString::new(output_path.to_str().unwrap()).unwrap();
            let mut error = ptr::null_mut();
            if LLVMTargetMachineEmitToFile(
                target_machine,
                self.module,
                output_cstr.as_ptr() as *mut _,
                LLVMCodeGenFileType::LLVMObjectFile,
                &mut error,
            ) != 0 {
                if !error.is_null() {
                    let error_str = CStr::from_ptr(error).to_string_lossy();
                    LLVMDisposeMessage(error);
                    return Err(CompileError::Generic(format!("Failed to write object file: {}", error_str)));
                }
            }
            
            // Cleanup
            LLVMDisposeTargetMachine(target_machine);
            LLVMDisposeMessage(target_triple);
            
            Ok(())
        }
        
        #[cfg(not(feature = "llvm"))]
        Err(CompileError::Generic("LLVM support not enabled".to_string()))
    }
    
    /// Write LLVM IR to file
    pub fn write_ir(&self, output_path: &PathBuf) -> Result<()> {
        #[cfg(feature = "llvm")]
        unsafe {
            let output_cstr = CString::new(output_path.to_str().unwrap()).unwrap();
            let mut error = ptr::null_mut();
            if LLVMPrintModuleToFile(self.module, output_cstr.as_ptr(), &mut error) != 0 {
                if !error.is_null() {
                    let error_str = CStr::from_ptr(error).to_string_lossy();
                    LLVMDisposeMessage(error);
                    return Err(CompileError::Generic(format!("Failed to write IR: {}", error_str)));
                }
            }
            Ok(())
        }
        
        #[cfg(not(feature = "llvm"))]
        Err(CompileError::Generic("LLVM support not enabled".to_string()))
    }
}

impl Drop for LLVMNativeGenerator {
    fn drop(&mut self) {
        #[cfg(feature = "llvm")]
        unsafe {
            LLVMDisposeBuilder(self.builder);
            LLVMDisposeModule(self.module);
            LLVMContextDispose(self.context);
        }
    }
}
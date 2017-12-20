use super::ast::*;

pub fn generate(prog: Program) -> String {
    match prog {
        Program { func } => func.into_iter().map(|a| { gen_function(a) }).collect(),
    }
}

fn gen_function(fun: Function) -> String {
    match fun {
        Function { name, statement } => format!(".global _{0}\n_{0}:\n{1}", name, gen_statement(statement)),
    }
}

fn gen_statement(stat: Statement) -> String {
    match stat {
        Statement::Return(exp) => format!("{}ret\n", gen_expression(exp)),
    }
}

fn gen_expression(exp: Expression) -> String {
    match exp {
        Expression::Int(val) => format!("movl ${}, %eax\n", val),
        Expression::UnOp(op, exp) => {
            let asm = match op {
                UnOp::Negation => "neg %eax\n",
                UnOp::BitComp => "not %eax\n",
                UnOp::LogicalNeg => "cmpl $0, %eax\nmovl $0, %eax\nsete %al\n",
            };
            format!("{}{}", gen_expression(*exp), asm)
        },
        _ => unimplemented!()
    }
}
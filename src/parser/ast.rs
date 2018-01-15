extern crate itertools;

use super::token::Token;
use super::token::Keyword;
use super::ops::*;

use std::vec::IntoIter;
use std::iter::Peekable;

use itertools::chain;

#[derive(Debug)]
pub struct Parser {
    tokens: Peekable<IntoIter<Token>>,
    peeked: Vec<Token>
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens: tokens.into_iter().peekable(), peeked: vec![] }
    }

    pub fn parse(&mut self) -> Program {
        self.parse_program()
    }

    fn next(&mut self) -> Option<Token> {
        if self.peeked.is_empty() {
            self.tokens.next()
        } else {
            self.peeked.pop()
        }
    }

    fn peek(&mut self) -> Option<Token> {
        if let Some(token) = self.next() {
            self.push(Some(token.clone()));
            Some(token)
        } else {
            None
        }
    }

    fn drop(&mut self, count: usize) {
        for _ in 0..count {
            self.next();
        }
    }

    fn push(&mut self, token: Option<Token>) {
        if let Some(t) = token {
            self.peeked.push(t);
        }
    }

    fn has_more(&mut self) -> bool {
        !self.peeked.is_empty() || self.tokens.peek().is_some()
    }

    fn next_token(&mut self) -> Token {
        self.next().expect("failed to parse")
    }

    fn match_token(&mut self, token: Token) -> Result<Token, String> {
        match self.next_token() {
            ref t if t == &token => Ok(token),
            other => Err(format!("Token {:?} not found, found {:?}", token, other))
        }
    }

    fn peek_token(&mut self, token: Token) -> Result<Token, String> {
        match self.peek() {
            Some(ref t) if t == &token => Ok(token),
            other => Err(format!("Token {:?} not found, found {:?}", token, other))
        }
    }

    fn match_keyword(&mut self, keyword: &Keyword) -> Result<(), String> {
        let token = self.next_token();
        match token {
            Token::Keyword(ref k) if k == keyword => Ok(()),
            other => Err(format!("Expected SemiColon, found {:?}", other))
        }
    }

    fn match_identifier(&mut self) -> Result<String, String> {
        match self.next_token() {
            Token::Identifier(n) => Ok(n),
            other => Err(format!("Expected Identifier, found {:?}", other))
        }
    }
}

impl Parser {
    fn parse_program(&mut self) -> Program {
        let mut functions = Vec::new();
        let globals = Vec::new();

        while self.has_more() {
            functions.push(self.parse_function().expect("Failed to parse function"))
        }

        Program { func: functions, globals: globals.into_iter().collect() }
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        self.match_keyword(&Keyword::Int)?;
        let name = self.match_identifier()?;
        self.match_token(Token::OpenParen)?;

        let arguments: Vec<String> = match self.peek() {
            Some(Token::CloseParen) => Vec::new(),
            _ => self.parse_arguments().expect("Failed to parse function arguments")
        };

        self.match_token(Token::CloseParen)?;
        self.match_token(Token::OpenBrace)?;

        let mut statements = vec![];
        let mut variables: Vec<String> = Vec::new();

        while let Err(_) = self.peek_token(Token::CloseBrace) {
            let statement = self.parse_statement(&chain(&variables, &arguments).collect::<Vec<&String>>());
            if let Statement::Declare(ref name, _) = statement {
                if variables.contains(name) || arguments.contains(name) {
                    return Err(format!("Variable alreay defined: {}", name))
                } else {
                    variables.push(name.clone());
                }
            }
            statements.push(statement);
        }

        self.match_token(Token::CloseBrace)?;

        Ok(Function { name, arguments, statements, variables })
    }

    fn parse_statement(&mut self, variables: &[&String]) -> Statement {
     match self.next() {
        Some(Token::Keyword(Keyword::Int)) => {
            let state = self.parse_declare(variables);
            self.match_token(Token::SemiColon).expect("Need SemiColon");
            state
        },
        Some(Token::Keyword(Keyword::Return)) => {
            let state = Ok(Statement::Return(self.parse_expression(variables)));
            self.match_token(Token::SemiColon).expect("Need SemiColon");
            state
        },
        Some(Token::Keyword(Keyword::If)) => self.parse_if_statement(variables),
        Some(Token::OpenBrace) => self.parse_compond_statement(variables),
        other => {
            self.push(other);
            let state  = Ok(Statement::Exp(self.parse_expression(variables)));
            self.match_token(Token::SemiColon).expect("Need SemiColon");
            state
        }
     }.expect("failed to parse")
    }


    fn parse_compond_statement(&mut self, variables: &[&String]) -> Result<Statement, String> {
        let mut statements  = vec![];
        while let Err(_) = self.peek_token(Token::CloseBrace) {
            statements.push(self.parse_statement(variables));
        }
        self.drop(1);
        Ok(Statement::Compound(statements))
    }

    fn parse_if_statement(&mut self, variables: &[&String]) -> Result<Statement, String> {
        match self.next_token() {
            Token::OpenParen => {
                let condition = self.parse_expression(variables);
                self.match_token(Token::CloseParen)?;
                let if_body = self.parse_statement(variables);
                self.match_token(Token::Keyword(Keyword::Else))?;
                let else_body = self.parse_statement(variables);
                Ok(Statement::If(condition, Box::new(if_body), Box::new(else_body)))
            },
            other => Err(format!("Expected OpenParen, found {:?}", other))
        }
    }

    fn parse_declare(&mut self, variables: &[&String]) -> Result<Statement, String> {
        match (self.next_token(), self.peek()) {
            (Token::Identifier(name), Some(Token::SemiColon)) => Ok(Statement::Declare(name, None)),
            (Token::Identifier(name), Some(Token::Assign)) => {
                self.drop(1);
                let exp = self.parse_expression(variables);
                Ok(Statement::Declare(name, Some(exp)))
            },
            other => Err(format!("Expected identifier, found {:?}", other))
        }
    }

    fn parse_assign_op(&mut self, bin_op: BinOp, name: &str, variables: &[&String]) -> Expression {
        self.check_var(variables, name);
        let exp = Expression::BinOp(
            bin_op,
            Box::new(Expression::Variable(name.to_string())),
            Box::new(self.parse_expression(variables))
        );
        Expression::Assign(name.to_string(), Box::new(exp))
    }

    fn parse_inc_op(&mut self, bin_op: BinOp, name: &str, variables: &[&String], postfix: bool) -> Expression {
        self.next();
        self.check_var(variables, name);
        let exp = Expression::BinOp(
            bin_op,
            Box::new(Expression::Variable(name.to_string())),
            Box::new(Expression::Int(1))
        );
        if postfix {
            Expression::AssignPostfix(name.to_string(), Box::new(exp))
        } else {
            Expression::Assign(name.to_string(), Box::new(exp))
        }

    }

    fn check_var(&self, variables: &[&String], name: &str) {
        if !variables.contains(&&name.to_string()) { panic!("Variable {} not defined", name) }
    }

    fn parse_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_comma_expression(variables)
    }

    fn parse_comma_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::Comma],
            variables,
            &Parser::parse_assignment_expression
        )
    }

    fn parse_assignment_expression(&mut self, variables: &[&String]) -> Expression {
        match (self.next(), self.next()) {
            (Some(Token::Identifier(name)), Some(Token::Assign)) => {
                self.check_var(variables, &name);
                let exp = self.parse_expression(variables);
                Expression::Assign(name.clone(), Box::new(exp))
            },
            (Some(Token::Identifier(name)), Some(Token::AssignAdd)) =>
                self.parse_assign_op(BinOp::Addition, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignSub)) =>
                self.parse_assign_op(BinOp::Subtraction, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignMul)) =>
                self.parse_assign_op(BinOp::Multiplication, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignDiv)) =>
                self.parse_assign_op(BinOp::Division, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignMod)) =>
                self.parse_assign_op(BinOp::Modulus, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignBitLeft)) =>
                self.parse_assign_op(BinOp::BitwiseLeft, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignBitRight)) =>
                self.parse_assign_op(BinOp::BitwiseRight, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignAnd)) =>
                self.parse_assign_op(BinOp::BitwiseAnd, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignOr)) =>
                self.parse_assign_op(BinOp::BitwiseOr, &name, variables),
            (Some(Token::Identifier(name)), Some(Token::AssignXor)) =>
                self.parse_assign_op(BinOp::BitwiseXor, &name, variables),
            (a, b) => {
                self.push(b);
                self.push(a);
                self.parse_conditional_expression(variables)
            }
        }
    }

    fn parse_conditional_expression(&mut self, variables: &[&String]) -> Expression {
        let mut term = self.parse_or_expression(variables);

        loop {
            match self.peek() {
                Some(Token::Question) => {
                    self.next();
                    let body = self.parse_expression(variables);
                    self.match_token(Token::Colon).expect("Expected a Colon");
                    let else_body = self.parse_expression(variables);
                    term = Expression::Ternary(Box::new(term), Box::new(body), Box::new(else_body))
                }
                _ => break
            }
        }

        term
    }

    fn parse_or_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::Or],
            variables,
             &Parser::parse_logical_and_expression
        )
    }

    fn parse_logical_and_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::And],
            variables,
            &Parser::parse_bitwise_or_expression
        )
    }

    fn parse_bitwise_or_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::BitwiseOr],
            variables,
            &Parser::parse_bitwise_xor_expression
        )
    }


    fn parse_bitwise_xor_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::BitwiseXor],
            variables,
            &Parser::parse_bitwise_and_expression
        )
    }

    fn parse_bitwise_and_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::BitwiseAnd],
            variables,
            &Parser::parse_equality_expression
        )
    }

    fn parse_equality_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::Equal, Token::NotEqual],
            variables,
            &Parser::parse_relational_expression
        )
    }

    fn parse_relational_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::LessThan, Token::GreaterThan, Token::LessThanOrEqual, Token::GreaterThanOrEqual],
            variables,
            &Parser::parse_bitshift_expression
        )
    }

    fn parse_bitshift_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::BitwiseLeft, Token::BitwiseRight],
            variables,
            &Parser::parse_additive_expression
        )
    }

    fn parse_additive_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::Negation, Token::Addition],
            variables,
            &Parser::parse_multiplicative_expression
        )
    }

    fn parse_multiplicative_expression(&mut self, variables: &[&String]) -> Expression {
        self.parse_gen_experssion(
            &[Token::Multiplication, Token::Division, Token::Modulus],
            variables,
            &Parser::parse_factor
        )
    }

    fn parse_factor(&mut self, variables: &[&String]) -> Expression {
        match (self.next(), self.peek()) {
            (Some(Token::Identifier(name)), Some(Token::Increment)) =>
                self.parse_inc_op(BinOp::Addition, &name, variables, true),
            (Some(Token::Identifier(name)), Some(Token::Decrement)) =>
                self.parse_inc_op(BinOp::Subtraction, &name, variables, true),
            (Some(Token::Increment), Some(Token::Identifier(name))) =>
                self.parse_inc_op(BinOp::Addition, &name, variables, false),
            (Some(Token::Decrement), Some(Token::Identifier(name))) =>
                self.parse_inc_op(BinOp::Subtraction, &name, variables, false),
            (Some(Token::OpenParen), _) => {
                let exp = self.parse_expression(variables);
                self.match_token(Token::CloseParen).expect("Must close the paren");
                exp
            },
            (Some(Token::Identifier(name)), _) => {
                match self.peek() {
                    Some(Token::OpenParen) => Expression::FunctionCall(name, self.parse_function_arguments(variables)),
                    _ => Expression::Variable(name)
                }
            },
            (Some(op @ Token::Negation), _) | (Some(op @ Token::LogicalNeg), _) | (Some(op @ Token::BitComp), _) => {
                let factor = self.parse_factor(variables);
                Expression::UnOp(op.into(), Box::new(factor))
            },
            (Some(Token::Literal(num)), _) => Expression::Int(num),
            (Some(Token::BitwiseAnd), _) => {
                match self.next() {
                    Some(Token::Identifier(name)) => Expression::VariableRef(name),
                    other => panic!("Only variables support &, found token: {:?}", other)
                }
            },
            op => panic!("Unknown token: {:?}", op),
        }
    }

    fn parse_function_arguments(&mut self, variables: &[&String]) -> Vec<Expression> {
        let mut arguments = vec![];
        self.next();
        while let Err(_) = self.peek_token(Token::CloseParen) {
            let exp = self.parse_assignment_expression(variables);
            arguments.push(exp);
            if let Some(Token::Comma) = self.peek() {
                self.next();
            }
        }
        self.next();
        arguments
    }

    fn parse_arguments(&mut self) -> Result<Vec<String>, String> {
        println!("parse_arguments");
        let mut arguments = Vec::new();
        while let Err(_) = self.peek_token(Token::CloseParen) {
            self.match_keyword(&Keyword::Int)?;
            let name = self.match_identifier()?;
            if arguments.contains(&name) {
                return Err(format!("Argument {} already defined", name))
            } else {
                arguments.push(name);
            }
            if let Some(Token::Comma) = self.peek() {
                self.next();
            }
        }
        Ok(arguments)
    }

    fn parse_gen_experssion<F>(&mut self, matching: &[Token], variables: &[&String], next: F) -> Expression
        where F: Fn(&mut Parser, &[&String]) -> Expression {
        let mut term = next(self, variables);

        loop {
            match self.peek() {
                Some(ref token) if matching.contains(token) => {
                    let op = self.next().unwrap().into();
                    let next_term = next(self, variables);
                    term = Expression::BinOp(op, Box::new(term), Box::new(next_term))
                }
                _ => break
            }
        }

        term
    }
}
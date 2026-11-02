use cel::{Context, Program, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OidcRuleContext {
    pub sub: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub roles: HashSet<String>,
    pub groups: HashSet<String>,
    pub claims: serde_json::Value,
}

pub fn compile_cel(expression: &str) -> Result<Program, anyhow::Error> {
    Program::compile(expression).map_err(|e| anyhow::anyhow!("Invalid CEL expression: {e}"))
}

pub fn evaluate_cel(
    program: &Program,
    context: &OidcRuleContext,
) -> Result<bool, anyhow::Error> {
    let mut ctx = Context::default();

    if let Some(ref sub) = context.sub {
        ctx.add_variable("sub", sub.clone())?;
    } else {
        ctx.add_variable("sub", "")?;
    }

    if let Some(ref name) = context.name {
        ctx.add_variable("name", name.clone())?;
    } else {
        ctx.add_variable("name", "")?;
    }

    if let Some(ref u) = context.preferred_username {
        ctx.add_variable("preferred_username", u.clone())?;
        ctx.add_variable("username", u.clone())?;
    } else {
        ctx.add_variable("preferred_username", "")?;
        ctx.add_variable("username", "")?;
    }

    if let Some(ref email) = context.email {
        ctx.add_variable("email", email.clone())?;
    } else {
        ctx.add_variable("email", "")?;
    }

    let roles_vec: Vec<String> = context.roles.iter().cloned().collect();
    ctx.add_variable("roles", roles_vec)?;

    let groups_vec: Vec<String> = context.groups.iter().cloned().collect();
    ctx.add_variable("groups", groups_vec)?;

    if !context.claims.is_null() {
        ctx.add_variable("claims", context.claims.clone())?;
    }

    let val = program.execute(&ctx).map_err(|e| anyhow::anyhow!("CEL execution error: {e}"))?;

    match val {
        Value::Bool(b) => Ok(b),
        other => anyhow::bail!("CEL expression did not return a boolean (returned: {other:?})"),
    }
}

pub fn test_cel_expression(
    expression: &str,
    sample_context: &OidcRuleContext,
) -> Result<bool, anyhow::Error> {
    let program = compile_cel(expression)?;
    evaluate_cel(&program, sample_context)
}

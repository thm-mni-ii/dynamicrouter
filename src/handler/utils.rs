use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

pub const GIB_IN_BYTE: u64 = 1024_u64.pow(3);

static MODEL_REF_REGEXP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(.*?)/(.*?)(?::(.*?))?(?:@(.*?))?$").unwrap());

#[derive(Debug, Clone)]
pub struct ModelRef {
    pub org: String,
    pub repo: String,
    pub rev: Option<String>,
    pub quant: Option<QuantType>,
}

impl FromStr for ModelRef {
    type Err = ModelRefParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = MODEL_REF_REGEXP
            .captures(s)
            .ok_or(ModelRefParseError::InvalidModelRef())?;
        Ok(ModelRef {
            org: res
                .get(1)
                .ok_or(ModelRefParseError::InvalidModelRef())?
                .as_str()
                .to_string(),
            repo: res
                .get(2)
                .ok_or(ModelRefParseError::InvalidModelRef())?
                .as_str()
                .to_string(),
            rev: res.get(3).map(|m| m.as_str().to_string()),
            quant: res.get(4).map(|m| m.as_str().parse()).transpose()?,
        })
    }
}

impl ModelRef {
    pub fn get_hf_id(&self) -> String {
        format!("{}/{}", self.org, self.repo)
    }
}

impl Display for ModelRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.org, self.repo)?;
        if let Some(rev) = &self.rev {
            write!(f, ":{rev}")?;
        }
        if let Some(quant) = &self.quant {
            write!(f, "@{quant}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum QuantType {
    FP32,
    FP16,
    FP8,
    FP4,
}

impl QuantType {
    pub fn get_byte_size(self) -> f64 {
        match self {
            QuantType::FP32 => 4.0,
            QuantType::FP16 => 2.0,
            QuantType::FP8 => 1.0,
            QuantType::FP4 => 0.5,
        }
    }
}

impl FromStr for QuantType {
    type Err = ModelRefParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "fp32" => QuantType::FP32,
            "fp16" => QuantType::FP16,
            "fp8" => QuantType::FP8,
            "fp4" => QuantType::FP4,
            _ => return Err(ModelRefParseError::InvalidQuant()),
        })
    }
}

impl Display for QuantType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                QuantType::FP32 => "fp32",
                QuantType::FP16 => "fp16",
                QuantType::FP8 => "fp8",
                QuantType::FP4 => "fp4",
            }
        )
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ModelRefParseError {
    #[error("failed to parse model ref")]
    InvalidModelRef(),
    #[error("failed to parse quant")]
    InvalidQuant(),
}

pub fn build_docker_args(model: &ModelRef) -> Vec<String> {
    let mut args = Vec::new();
    args.append(&mut vec!["--model-id".to_string(), model.get_hf_id()]);

    if let Some(rev) = &model.rev {
        args.append(&mut vec!["--revision".to_string(), rev.to_string()]);
    }

    match &model.quant {
        Some(QuantType::FP8) => args.append(&mut vec!["--quantize".to_string(), "fp8".to_string()]),
        Some(QuantType::FP4) => args.append(&mut vec![
            "--quantize".to_string(),
            "bitsandbytes-fp4".to_string(),
        ]),
        _ => {}
    };

    args
}

use crate::config::CONFIG;
use crate::handler::huggingface::HfModel;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

static MODEL_REF_REGEXP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.*?)/(.*?)(?::(.*?))?(?:/(.*?))?(?:@(.*?))?$").unwrap());
static QUANT_REGEXP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([Gg])(\d{1,2})$").unwrap());

#[derive(Debug, Clone)]
pub struct ModelRef {
    pub org: String,
    pub repo: String,
    pub rev: Option<String>,
    pub path: Option<String>,
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
            path: res.get(4).map(|m| m.as_str().to_string()),
            quant: res.get(5).map(|m| m.as_str().parse()).transpose()?,
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
        if let Some(path) = &self.path {
            write!(f, "/{path}")?;
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
    Gguf(u8),
}

impl QuantType {
    pub fn get_byte_size(self) -> f64 {
        match self {
            QuantType::FP32 => 4.0,
            QuantType::FP16 => 2.0,
            QuantType::FP8 => 1.0,
            QuantType::FP4 => 0.5,
            QuantType::Gguf(q) => f64::from(q) / 8.0,
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
            oe => {
                let res = QUANT_REGEXP
                    .captures(oe)
                    .ok_or(ModelRefParseError::InvalidQuant())?;
                match res
                    .get(1)
                    .ok_or(ModelRefParseError::InvalidQuant())?
                    .as_str()
                    .to_uppercase()
                    .as_str()
                {
                    "G" => QuantType::Gguf(
                        res.get(2)
                            .ok_or(ModelRefParseError::InvalidQuant())?
                            .as_str()
                            .parse()
                            .map_err(|_| ModelRefParseError::InvalidQuant())?,
                    ),
                    _ => return Err(ModelRefParseError::InvalidModelRef()),
                }
            }
        })
    }
}

impl Display for QuantType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                QuantType::FP32 => "fp32".to_string(),
                QuantType::FP16 => "fp16".to_string(),
                QuantType::FP8 => "fp8".to_string(),
                QuantType::FP4 => "fp4".to_string(),
                QuantType::Gguf(size) => format!("G{size}"),
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

const GIB_IN_BYTE: u64 = 1024_u64.pow(3);

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize, Default)]
#[repr(transparent)]
pub struct ByteSize(u64);

impl ByteSize {
    #[inline]
    pub fn from_bytes(bytes: u64) -> ByteSize {
        ByteSize(bytes)
    }

    #[inline]
    pub fn from_gibibyte(gibibyte: u64) -> ByteSize {
        ByteSize(gibibyte * GIB_IN_BYTE)
    }

    #[inline]
    pub fn to_gibibytes(self) -> f64 {
        self.0 as f64 / GIB_IN_BYTE as f64
    }

    #[inline]
    pub fn bytes(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn factor(self, factor: f64) -> ByteSize {
        ByteSize::from_bytes((self.0 as f64 * factor).ceil() as u64)
    }
}

pub fn calculate_needed_space(parameters: u64, quant: QuantType) -> ByteSize {
    let fb = parameters as f64 * quant.get_byte_size();
    ByteSize::from_bytes(fb.ceil() as u64)
}

#[derive(Debug, Copy, Clone)]
pub struct ModelSizes {
    pub gpu: ByteSize,
    pub storage: ByteSize,
}

impl ModelSizes {
    pub async fn get(model: &ModelRef) -> Result<ModelSizes, anyhow::Error> {
        let model_information = HfModel::get(model.get_hf_id()).await?;
        let parameter_override = CONFIG
            .models
            .get(&model.to_string())
            .and_then(|model| model.parameter_count);

        Ok(ModelSizes {
            gpu: calculate_needed_space(
                parameter_override
                    .or(model_information.parameter_count)
                    .ok_or(ModelSizeError::GetParameter())?,
                model.quant.unwrap_or(QuantType::FP16),
            ),
            storage: match parameter_override {
                Some(parameters) => calculate_needed_space(
                    parameters,
                    match model.quant {
                        Some(QuantType::Gguf(q)) => QuantType::Gguf(q),
                        _ => QuantType::FP16,
                    },
                ),
                None => ByteSize::from_bytes(
                    model_information
                        .storage_size
                        .ok_or(ModelSizeError::GetStorageSize())?,
                ),
            },
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ModelSizeError {
    #[error("missing total safetensors parameter count")]
    GetParameter(),
    #[error("missing storage size")]
    GetStorageSize(),
}

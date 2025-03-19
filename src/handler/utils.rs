use crate::handler::huggingface::HfModel;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::num::TryFromIntError;
use std::str::FromStr;

static MODEL_REF_REGEXP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.*?)/(.*?)(?::(.*?))?(?:/(.*?))?(?:#(.*?))?(?:@(.*?))?$").unwrap());
static QUANT_REGEXP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([Gg])(\d{1,2})$").unwrap());
static PARAMETER_COUNT_EXTRACTION_REGEXP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d+[BMbm]").unwrap());

#[derive(Debug, Clone)]
pub struct ModelRef {
    pub org: String,
    pub repo: String,
    pub rev: Option<String>,
    pub path: Option<String>,
    pub quant: Option<QuantType>,
    pub param_count_inferred: Option<ParameterCount>,
    pub param_count_explicit: Option<ParameterCount>,
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
            param_count_explicit: res.get(5).map(|m| m.as_str().parse()).transpose()?,
            quant: res.get(6).map(|m| m.as_str().parse()).transpose()?,
            param_count_inferred: res
                .get(0)
                .and_then(|m| PARAMETER_COUNT_EXTRACTION_REGEXP.find(m.as_str()))
                .map(|m| m.as_str().parse())
                .transpose()?,
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
        if let Some(param_count_explicit) = &self.param_count_explicit {
            write!(f, "#{param_count_explicit}")?;
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
    #[error("failed to parse parameter count")]
    InvalidParameterCount(),
}

const GIB_IN_BYTE: u64 = 1024_u64.pow(3);

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize, Default)]
#[repr(transparent)]
pub struct ByteSize(u64);

impl ByteSize {
    #[inline]
    pub fn from_bytes(bytes: u64) -> Self {
        ByteSize(bytes)
    }

    #[inline]
    pub fn from_gibibyte(gibibyte: u64) -> Self {
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

impl TryFrom<i64> for ByteSize {
    type Error = TryFromIntError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(ByteSize(u64::try_from(value)?))
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize, Default)]
#[repr(transparent)]
pub struct ParameterCount(u64);

static PARAMETER_COUNT_REGEXP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d+)([BMbm])$").unwrap());

impl FromStr for ParameterCount {
    type Err = ModelRefParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pcc = PARAMETER_COUNT_REGEXP
            .captures(s)
            .ok_or(ModelRefParseError::InvalidParameterCount())?;
        let number = pcc
            .get(1)
            .ok_or(ModelRefParseError::InvalidParameterCount())?
            .as_str()
            .parse::<u64>()
            .map_err(|_| ModelRefParseError::InvalidParameterCount())?;
        let suffix = pcc
            .get(2)
            .ok_or(ModelRefParseError::InvalidParameterCount())?
            .as_str();
        let multiplier = match suffix.to_uppercase().as_str() {
            "M" => 10u64.pow(6),
            "B" => 10u64.pow(9),
            _ => return Err(ModelRefParseError::InvalidParameterCount()),
        };
        Ok(ParameterCount(number * multiplier))
    }
}

impl TryFrom<i64> for ParameterCount {
    type Error = TryFromIntError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(ParameterCount(u64::try_from(value)?))
    }
}

impl Display for ParameterCount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn calculate_needed_space(parameters: ParameterCount, quant: QuantType) -> ByteSize {
    let fb = parameters.0 as f64 * quant.get_byte_size();
    ByteSize::from_bytes(fb.ceil() as u64)
}

#[derive(Debug, Copy, Clone)]
pub struct ModelSizes {
    pub gpu: ByteSize,
    pub storage: ByteSize,
}

impl ModelSizes {
    /*
    gpu:
    override
    explicit
    hf
    implicit
    */
    /*
    hf
    override
    explicit
    implicit
     */
    pub async fn get(model: &ModelRef) -> Result<ModelSizes, anyhow::Error> {
        let model_information = HfModel::get(model.get_hf_id()).await?;

        Ok(ModelSizes {
            gpu: calculate_needed_space(
                model
                    .param_count_explicit
                    .or(model_information.parameter_count)
                    .or(model.param_count_inferred)
                    .ok_or(ModelSizeError::GetParameter())?,
                model.quant.unwrap_or(QuantType::FP16),
            ),
            storage: model_information
                .storage_size
                .or_else(|| {
                    Some(calculate_needed_space(
                        match model
                            .param_count_explicit
                            .or(model_information.parameter_count)
                            .or(model.param_count_inferred)
                        {
                            Some(params) => params,
                            None => return None,
                        },
                        match model.quant {
                            Some(QuantType::Gguf(q)) => QuantType::Gguf(q),
                            _ => QuantType::FP16,
                        },
                    ))
                })
                .ok_or(ModelSizeError::GetStorageSize())?,
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

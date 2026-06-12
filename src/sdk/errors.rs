use ambient_auction_api::error::AuctionError;
use solana_sdk::{instruction::InstructionError, transaction::TransactionError};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuctionProgramError {
    Known(AuctionError),
    UnknownCustom(u32),
    Runtime(InstructionError),
}

impl AuctionProgramError {
    pub fn from_instruction_error(error: InstructionError) -> Self {
        match error {
            InstructionError::Custom(code) => AuctionError::try_from_code(code)
                .map(Self::Known)
                .unwrap_or(Self::UnknownCustom(code)),
            error => Self::Runtime(error),
        }
    }

    pub fn auction_error(&self) -> Option<AuctionError> {
        match self {
            Self::Known(error) => Some(*error),
            Self::UnknownCustom(_) | Self::Runtime(_) => None,
        }
    }
}

impl Display for AuctionProgramError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(error) => write!(f, "{}: {}", error.name(), error.message()),
            Self::UnknownCustom(code) => write!(f, "Unknown auction custom error code {code}"),
            Self::Runtime(error) => write!(f, "Solana instruction error: {error}"),
        }
    }
}

impl std::error::Error for AuctionProgramError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionTransactionError {
    pub instruction_index: u8,
    pub error: AuctionProgramError,
}

impl AuctionTransactionError {
    pub fn from_transaction_error(error: TransactionError) -> Option<Self> {
        match error {
            TransactionError::InstructionError(instruction_index, error) => Some(Self {
                instruction_index,
                error: AuctionProgramError::from_instruction_error(error),
            }),
            _ => None,
        }
    }
}

impl Display for AuctionTransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "auction instruction {} failed: {}",
            self.instruction_index, self.error
        )
    }
}

impl std::error::Error for AuctionTransactionError {}

pub fn decode_instruction_error(error: InstructionError) -> AuctionProgramError {
    AuctionProgramError::from_instruction_error(error)
}

pub fn decode_transaction_error(error: TransactionError) -> Option<AuctionTransactionError> {
    AuctionTransactionError::from_transaction_error(error)
}

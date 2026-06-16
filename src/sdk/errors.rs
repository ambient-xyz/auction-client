use ambient_auction_api::error::AuctionError;
use solana_sdk::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    transaction::TransactionError,
};
use std::fmt::{Display, Formatter};

fn fmt_auction_error(error: AuctionError, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}: {}", error.name(), error.message())
}

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

    pub fn custom_code(&self) -> Option<u32> {
        match self {
            Self::Known(error) => Some(error.code()),
            Self::UnknownCustom(code) => Some(*code),
            Self::Runtime(_) => None,
        }
    }
}

impl Display for AuctionProgramError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(error) => fmt_auction_error(*error, f),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInstructionError {
    pub instruction_index: u8,
    pub program_id: Option<Pubkey>,
    pub error: InstructionError,
    pub auction_error: Option<AuctionProgramError>,
}

impl DecodedInstructionError {
    pub fn from_instruction_error(
        instruction_index: u8,
        error: InstructionError,
        instructions: &[Instruction],
        auction_program_id: Pubkey,
    ) -> Self {
        let program_id = instructions
            .get(usize::from(instruction_index))
            .map(|instruction| instruction.program_id);
        let auction_error = (program_id == Some(auction_program_id))
            .then(|| AuctionProgramError::from_instruction_error(error.clone()));

        Self {
            instruction_index,
            program_id,
            error,
            auction_error,
        }
    }

    pub fn auction_program_error(&self) -> Option<&AuctionProgramError> {
        self.auction_error.as_ref()
    }

    pub fn auction_error(&self) -> Option<AuctionError> {
        self.auction_error
            .as_ref()
            .and_then(AuctionProgramError::auction_error)
    }

    pub fn custom_code(&self) -> Option<u32> {
        match &self.error {
            InstructionError::Custom(code) => Some(*code),
            _ => None,
        }
    }
}

impl Display for DecodedInstructionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(auction_error) = &self.auction_error {
            return match self.program_id {
                Some(program_id) => write!(
                    f,
                    "auction instruction {} ({program_id}) failed: {auction_error}",
                    self.instruction_index
                ),
                None => write!(
                    f,
                    "auction instruction {} failed: {auction_error}",
                    self.instruction_index
                ),
            };
        }

        match self.program_id {
            Some(program_id) => write!(
                f,
                "instruction {} ({program_id}) failed: {}",
                self.instruction_index, self.error
            ),
            None => write!(
                f,
                "instruction {} failed: {} (program id unavailable)",
                self.instruction_index, self.error
            ),
        }
    }
}

impl std::error::Error for DecodedInstructionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedTransactionError {
    Instruction(DecodedInstructionError),
    Transaction(TransactionError),
}

impl DecodedTransactionError {
    pub fn from_transaction_error(
        error: TransactionError,
        instructions: &[Instruction],
        auction_program_id: Pubkey,
    ) -> Self {
        match error {
            TransactionError::InstructionError(instruction_index, error) => {
                Self::Instruction(DecodedInstructionError::from_instruction_error(
                    instruction_index,
                    error,
                    instructions,
                    auction_program_id,
                ))
            }
            error => Self::Transaction(error),
        }
    }

    pub fn instruction_index(&self) -> Option<u8> {
        match self {
            Self::Instruction(error) => Some(error.instruction_index),
            Self::Transaction(_) => None,
        }
    }

    pub fn program_id(&self) -> Option<Pubkey> {
        match self {
            Self::Instruction(error) => error.program_id,
            Self::Transaction(_) => None,
        }
    }

    pub fn instruction_error(&self) -> Option<&InstructionError> {
        match self {
            Self::Instruction(error) => Some(&error.error),
            Self::Transaction(_) => None,
        }
    }

    pub fn transaction_error(&self) -> Option<&TransactionError> {
        match self {
            Self::Instruction(_) => None,
            Self::Transaction(error) => Some(error),
        }
    }

    pub fn auction_program_error(&self) -> Option<&AuctionProgramError> {
        match self {
            Self::Instruction(error) => error.auction_program_error(),
            Self::Transaction(_) => None,
        }
    }

    pub fn auction_error(&self) -> Option<AuctionError> {
        match self {
            Self::Instruction(error) => error.auction_error(),
            Self::Transaction(_) => None,
        }
    }

    pub fn custom_code(&self) -> Option<u32> {
        match self {
            Self::Instruction(error) => error.custom_code(),
            Self::Transaction(_) => None,
        }
    }
}

impl Display for DecodedTransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Instruction(error) => Display::fmt(error, f),
            Self::Transaction(error) => write!(f, "transaction failed: {error}"),
        }
    }
}

impl std::error::Error for DecodedTransactionError {}

pub fn decode_instruction_error(error: InstructionError) -> AuctionProgramError {
    AuctionProgramError::from_instruction_error(error)
}

pub fn decode_transaction_error(error: TransactionError) -> Option<AuctionTransactionError> {
    AuctionTransactionError::from_transaction_error(error)
}

pub fn decode_transaction_error_with_instructions(
    error: TransactionError,
    instructions: &[Instruction],
    auction_program_id: Pubkey,
) -> DecodedTransactionError {
    DecodedTransactionError::from_transaction_error(error, instructions, auction_program_id)
}

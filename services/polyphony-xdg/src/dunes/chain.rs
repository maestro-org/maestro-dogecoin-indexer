use serde::{Deserialize, Serialize};
use {super::*, clap::ValueEnum};

#[derive(Default, ValueEnum, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Chain {
    #[default]
    #[clap(alias("main"))]
    Mainnet,
    #[clap(alias("test"))]
    Testnet,
    Signet,
    Regtest,
}

impl Chain {
    pub(crate) fn first_dune_height(self) -> u32 {
        match self {
            Self::Mainnet => 5084000,
            Self::Regtest => 0,
            Self::Signet => 0,
            Self::Testnet => 4250000,
        }
    }
}

impl Display for Chain {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mainnet => "mainnet",
                Self::Regtest => "regtest",
                Self::Signet => "signet",
                Self::Testnet => "testnet",
            }
        )
    }
}

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, PickFirst};
use utoipa::ToSchema;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct MempoolTransactionDetails {
    /// Number of ancestor transactions
    #[serde(alias = "ancestorcount")]
    #[schema(example = 1)]
    pub ancestor_count: u64,

    /// Fees associated with ancestor transactions (in Koinu)
    #[serde(alias = "ancestorfees")]
    #[schema(example = 289000000)]
    pub ancestor_fees: u64,

    /// Size of ancestor transactions (in bytes)
    #[serde(alias = "ancestorsize")]
    #[schema(example = 259)]
    pub ancestor_size: u64,

    /// Current priority of the transaction
    #[serde(alias = "currentpriority")]
    #[schema(example = 45122401597.51786)]
    pub current_priority: f64,

    /// Dependencies of the transaction
    #[schema(example = json!([]))]
    pub depends: Vec<String>,

    /// Number of descendant transactions
    #[serde(alias = "descendantcount")]
    #[schema(example = 1)]
    pub descendant_count: u64,

    /// Fees associated with descendant transactions (in Koinu)
    #[serde(alias = "descendantfees")]
    #[schema(example = 289000000)]
    pub descendant_fees: u64,

    /// Size of descendant transactions (in bytes)
    #[serde(alias = "descendantsize")]
    #[schema(example = 259)]
    pub descendant_size: u64,

    /// Base fee of the transaction (in Dogecoin)
    #[serde(alias = "fee")]
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    #[schema(example = "2.89", value_type = String)]
    pub fee: f64,

    /// Block height at which the transaction was included
    #[schema(example = 5337265)]
    pub height: u64,

    /// Modified fee of the transaction (in Dogecoin)
    #[serde(alias = "modifiedfee")]
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    #[schema(example = "2.89", value_type = String)]
    pub modified_fee: f64,

    /// Size of the transaction (in bytes)
    #[serde(alias = "size")]
    #[schema(example = 259)]
    pub size: u64,

    /// Starting priority of the transaction
    #[serde(alias = "startingpriority")]
    #[schema(example = 45122401597.51786)]
    pub starting_priority: f64,

    /// Time when the transaction was added to the mempool
    #[schema(example = 1723735497)]
    pub time: u64,
}

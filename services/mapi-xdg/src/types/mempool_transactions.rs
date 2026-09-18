use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MempoolTransactions(Vec<String>);

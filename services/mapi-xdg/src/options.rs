use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    GenerateOpenApi,
    Dogecoin,
    DogecoinTestnet,
}

#[derive(Debug, Parser)]
pub struct Options {
    #[clap(
        short = 'l',
        long = "listen-address",
        default_value = "0.0.0.0:3000",
        env = "LISTEN_ADDRESS"
    )]
    pub listen_address: SocketAddr,

    #[clap(short = 'm', long = "mode", env = "MODE")]
    pub mode: Mode,

    #[clap(short = 'a', long = "node-address", env = "NODE_ADDRESS")]
    /// The address of the Dogecoin node
    pub node_address: String,

    #[clap(
        short = 'u',
        long = "node-user",
        default_value = "maestro",
        env = "NODE_USER"
    )]
    /// The username to authenticate with the Dogecoin node
    pub node_user: String,

    #[clap(
        short = 'p',
        long = "node-password",
        default_value = "maestro",
        env = "NODE_PASSWORD"
    )]
    /// The password to authenticate with the Dogecoin node
    pub node_password: String,

    #[clap(
        long = "tikv-address",
        default_value = "127.0.0.1:2379",
        env = "TIKV_PD_CLIENT"
    )]
    /// TiKV PD client address
    pub tikv_address: String,

    #[clap(long = "redis", env = "REDIS")]
    /// Address of the redis cluster, for example: 'redis://localhost:6379'
    pub redis: String,
}

impl Options {
    pub fn parse() -> Self {
        <Self as clap::Parser>::parse()
    }
}

#[path = "planning/command_builder.rs"]
mod command_builder;
#[path = "planning/strategy.rs"]
mod strategy;

pub(in crate::modules::compress_videos::processor) use self::{
    command_builder::build_encode_command,
    strategy::{EncodePlan, build_plan},
};

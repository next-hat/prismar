pub mod schema;
pub mod namespace;
pub use namespace::*;
pub mod cargo;
pub use cargo::*;

#[allow(dead_code)]
pub type PrismaWhereInput = prismar::PrismaWhereInput;
#[allow(dead_code)]
pub type PrismaReadManyInput = prismar::PrismaReadManyInput;

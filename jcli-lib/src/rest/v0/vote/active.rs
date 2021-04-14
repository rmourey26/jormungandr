use super::{committees::Committees, plans::Plans};
use crate::rest::Error;
#[cfg(feature = "structopt")]
use structopt::StructOpt;

#[cfg_attr(
    feature = "structopt",
    derive(StructOpt),
    structopt(rename_all = "kebab-case")
)]
pub enum Active {
    /// Committee members
    Committees(Committees),
    /// Active vote plans
    Plans(Plans),
}

impl Active {
    pub fn exec(self) -> Result<(), Error> {
        match self {
            Active::Committees(committees) => committees.exec(),
            Active::Plans(plans) => plans.exec(),
        }
    }
}

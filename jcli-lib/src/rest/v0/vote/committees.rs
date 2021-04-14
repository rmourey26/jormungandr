use crate::rest::{Error, RestArgs};
use crate::utils::OutputFormat;
#[cfg(feature = "structopt")]
use structopt::StructOpt;

#[cfg_attr(
    feature = "structopt",
    derive(StructOpt),
    structopt(rename_all = "kebab-case")
)]
pub enum Committees {
    /// Get committee members list
    Get {
        #[cfg_attr(feature = "structopt", structopt(flatten))]
        args: RestArgs,
        #[cfg_attr(feature = "structopt", structopt(flatten))]
        output_format: OutputFormat,
    },
}

impl Committees {
    pub fn exec(self) -> Result<(), Error> {
        let Committees::Get {
            args,
            output_format,
        } = self;
        let response = args
            .client()?
            .get(&["v0", "vote", "active", "committees"])
            .execute()?
            .json()?;
        let formatted = output_format.format_json(response)?;
        println!("{}", formatted);
        Ok(())
    }
}

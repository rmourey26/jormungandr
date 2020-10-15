use super::Error;
use crate::jcli_app::utils::OutputFormat;
use chain_vote::{OpeningVoteKey, Tally};
use serde::Serialize;
use std::{
    fs::File,
    io::{stdin, Read},
    path::PathBuf,
};
use structopt::StructOpt;

/// Create the decryption share for decrypting the tally of private voting.
/// The outputs are provided as hex-encoded byte sequences.
#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct TallyDecryptionShare {
    /// The path to hex-encoded encrypted tally state. If this parameter is not
    /// specified, the encrypted tally state will be read from the standard
    /// input.
    encrypted_tally: Option<PathBuf>,
    /// The path to hex-encoded decryption key.
    decryption_key: PathBuf,
    #[structopt(flatten)]
    output_format: OutputFormat,
}

#[derive(Serialize)]
struct Output {
    state: String,
    share: String,
}

impl TallyDecryptionShare {
    pub fn exec(&self) -> Result<(), Error> {
        let encrypted_tally_hex = if let Some(encrypted_tally_path) = &self.encrypted_tally {
            let mut data = Vec::new();
            File::open(encrypted_tally_path)?.read_to_end(&mut data)?;
            data
        } else {
            let mut data = Vec::new();
            stdin().read_to_end(&mut data)?;
            data
        };
        let encrypted_tally_bytes = hex::decode(encrypted_tally_hex)?;
        let encrypted_tally =
            Tally::from_bytes(&encrypted_tally_bytes).ok_or(Error::EncryptedTallyRead)?;

        let decryption_key = {
            let mut data = Vec::new();
            File::open(&self.decryption_key)?.read_to_end(&mut data)?;
            let mut bytes = [0u8; 32];
            hex::decode_to_slice(data, &mut bytes as &mut [u8])?;
            OpeningVoteKey::from_bytes(&bytes).ok_or(Error::DecryptionKeyRead)?
        };

        let (state, share) = encrypted_tally.finish(&decryption_key);

        let output = self
            .output_format
            .format_json(serde_json::to_value(Output {
                state: hex::encode(state.to_bytes()),
                share: hex::encode(share.to_bytes()),
            })?)?;

        println!("{}", output);

        Ok(())
    }
}

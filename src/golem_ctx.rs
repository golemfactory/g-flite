pub use failure::Error;
use std::path::PathBuf;

pub struct GolemCtx {
    pub rpc_addr: (String, u16),
    pub data_dir: PathBuf,
}

impl GolemCtx {
    pub fn connect_to_app(
        &mut self,
    ) -> Result<(actix::SystemRunner, impl actix_wamp::RpcEndpoint + Clone), Error> {
        let mut sys = actix::System::new("golemcli");

        let data_dir = self.data_dir.clone();

        let auth_method =
            actix_wamp::challenge_response_auth(move |auth_id| -> Result<_, std::io::Error> {
                let secret_file_path = data_dir.join(format!("crossbar/secrets/{}.tck", auth_id));
                log::debug!("reading secret from: {}", secret_file_path.display());
                Ok(std::fs::read(secret_file_path)?)
            });

        let (address, port) = &self.rpc_addr;

        let endpoint = sys.block_on(
            actix_wamp::SessionBuilder::with_auth("golem", "golemcli", auth_method)
                .create_wss(address, *port),
        )?;

        Ok((sys, endpoint))
    }
}

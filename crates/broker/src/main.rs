fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref().unwrap_or("run") {
        "run" => dictation_broker::server::run(&dictation_api::default_home()),
        "install" => {
            println!(
                "{}",
                dictation_broker::service::install(args.next().as_deref() != Some("--no-start"))?
                    .display()
            );
            Ok(())
        }
        "uninstall" => dictation_broker::service::uninstall(),
        "status" => {
            let connection = dictation_api::Connection::connect(None)?;
            connection.request(dictation_protocol::Request::Ping)?;
            println!("running");
            Ok(())
        }
        "endpoint" => {
            println!("{}", dictation_api::default_endpoint().display());
            Ok(())
        }
        _ => Err(
            "usage: dictation-broker [run|install [--no-start]|uninstall|status|endpoint]".into(),
        ),
    }
}

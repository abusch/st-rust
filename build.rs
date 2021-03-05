use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["protos/bep.proto", "protos/local.proto"], &["protos/"])?;
    Ok(())
}

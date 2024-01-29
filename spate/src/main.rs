use spate_bencode::Value;
use spate_metainfo::MetaInfo;
use std::path::PathBuf;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufReader, BufWriter},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/ubuntu-23.10.1-desktop-amd64.iso.torrent");
    let file = File::open(path).await?;
    let mut reader = BufReader::new(file);
    let result = Value::decode(&mut reader).await?;
    let meta = MetaInfo::try_from(&result).unwrap();

    println!("Meta: {:?}", meta);

    match result {
        Value::Dict(value) => {
            let val: Option<&Value> = value.get(&Value::Bytes("announce".into()));
            println!("Announce {:?}", val);

            let mut buf = Vec::<u8>::new();
            let mut writer = BufWriter::new(&mut buf);
            val.unwrap().encode(&mut writer).await?;
            writer.flush().await?;
            println!("Encoded {:?}", String::from_utf8_lossy(&buf));
        }
        _ => {
            println!("Expected dict");
        }
    }

    Ok(())
}

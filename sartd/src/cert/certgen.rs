use std::{ops::Add, fs::File, io::Write};

use clap::Parser;
use rcgen::{CertificateParams, Certificate, ExtendedKeyUsagePurpose, KeyUsagePurpose, SerialNumber};
use time::{OffsetDateTime, Duration};

const DEFAULT_SERVICE_URL: &'static str = "sart-webhook-service.kube-system.svc";
const DEFAULT_OUTPUT_DIR: &'static str = ".";

#[derive(Parser)]
struct Args {
	/// TLS hostname
	#[arg(long)]
	#[clap(default_value = DEFAULT_SERVICE_URL)]
	host: String,

	#[arg(long)]
	#[clap(default_value = DEFAULT_OUTPUT_DIR)]
	out_dir: String,

}

fn main() -> anyhow::Result<()> {

	println!("Generate certificate and key files");

	let arg = Args::parse();

	let mut params = CertificateParams::new(vec![arg.host]);
	params.not_before = OffsetDateTime::now_utc();
	params.not_after = OffsetDateTime::now_utc().add(Duration::hours(36500*24));
	params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
	params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

	let cert = Certificate::from_params(params)?;
	
	let certificate_pem = cert.serialize_pem()?;
	let private_key_pem = cert.serialize_private_key_pem();

	let mut certificate_pem_file = File::create(format!("{}/tls.cert", arg.out_dir))?;
	let c_metadata = certificate_pem_file.metadata()?;
	let mut c_permissions = c_metadata.permissions();
	c_permissions.set_readonly(true);

	let mut private_key_pem_file = File::create(format!("{}/tls.key", arg.out_dir))?;
	let p_metadata = private_key_pem_file.metadata()?;
	let mut p_permissions = p_metadata.permissions();
	p_permissions.set_readonly(true);

	certificate_pem_file.write_all(certificate_pem.as_bytes())?;
	private_key_pem_file.write_all(private_key_pem.as_bytes())?;

	Ok(())
}

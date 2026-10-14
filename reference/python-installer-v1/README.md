# Retired Python installer reference

Seven exact Python installer/publisher/package/test sources were preserved before native migration. They are historical reference, not an active installer, supported fallback, builder input or recipient dependency.

`MANIFEST.json` binds original source-relative paths and bytes. Manifest SHA-256: `dfe5f852f6c57dd3b0b2c2531d96ab44acbeec4b378b9bfed7ec2bac9f256489`.

Active installation is now implemented by the independent Rust crate `next/installer`, sharing existing native service-control APIs. Artifact publication invariants map to native fixtures/tests; wrapper helper/interpreter mechanisms are replaced by direct native APIs. Builder-only `next/dagger/package.py` remains solely for payload metadata generation/verification and has no host publication, lifecycle or model-provisioning behavior.

Do not reactivate these scripts, install them on the host or use archived evidence as validation of new native candidates. Current plans, native test mapping, recipient ABI/dependency checks and independent assurance govern the replacement. This reference contains no models, credentials, installed state or normal-host approval.

// napi-build generates the napi glue code for the Node.js addon.
// This file must exist in the nodejs/ crate root — napi-rs requires it.
extern crate napi_build;

fn main() {
    napi_build::setup();
}

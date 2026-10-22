//! Generates this crate's private string catalog, `res::str::…`, from `resource/locales`
//! (https://daybrite.dev/docs/how-to/localization "Private catalogs").
fn main() {
    day_build::generate_locales().expect("day-build: localization codegen");
}

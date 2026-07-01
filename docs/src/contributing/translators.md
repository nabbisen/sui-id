# Adding or updating translations

sui-id uses a compile-time exhaustive translation system in `crates/sui-id-i18n`.
Missing translations are **compile errors**, not runtime warnings.

## How the system works

Every translatable string is a field on `Strings` (in `strings.rs`).
Each supported locale has a `static STRINGS_*` constant that provides
every field. Adding a field to `Strings` makes all locale files fail
to compile until they supply the new value.

Each locale lives in a **self-contained file** under `crates/sui-id-i18n/src/locale/`:

```
crates/sui-id-i18n/src/
  strings.rs          — Strings struct (add fields here)
  formatters.rs       — Formatters struct + shared helpers
  locale.rs           — module declarations and re-exports
  locale/
    en.rs             — English (STRINGS_EN + FORMATTERS_EN)
    ja.rs             — Japanese (STRINGS_JA + FORMATTERS_JA)
    zh_hans.rs        — Chinese Simplified (STRINGS_ZH_HANS + FORMATTERS_ZH_HANS)
    zh_hant.rs        — Chinese Traditional stub — see file for contribution guide
```

## Updating an existing translation

Open the relevant file under `locale/` and edit the field values.
Run `cargo test -p sui-id-i18n` to verify.

## Adding a new locale

1. **Create the locale file** — copy `locale/en.rs` to `locale/xx.rs`
   and translate every string field. Also write the per-locale formatter
   functions in the same file following the `en_fmt_*` pattern.

2. **Declare the module** in `locale.rs`:

   ```rust
   pub mod ko;
   pub use ko::{FORMATTERS_KO, STRINGS_KO};
   ```

3. **Add the variant** to `Locale` in `lib.rs`:

   ```rust
   #[serde(rename = "ko")]
   Ko,
   ```

4. **Wire the four match arms** — `tag()`, `native_name()`, `strings()`,
   `formatters()`, `direction()`, and `parse()`:

   ```rust
   // tag()
   Self::Ko => "ko",

   // native_name()
   Self::Ko => "한국어",

   // direction() — use "rtl" for Arabic, Hebrew, etc.
   Self::Ko => "ltr",

   // strings() / formatters() / parse()
   Self::Ko => &STRINGS_KO,
   Self::Ko => &FORMATTERS_KO,
   ("ko", _) => Some(Locale::Ko),
   ```

5. **Add to `Locale::ALL`** once the translation is complete and reviewed:

   ```rust
   pub const ALL: &'static [Locale] = &[Locale::Ja, Locale::En, Locale::Ko];
   ```

6. **Run `cargo check -p sui-id-i18n`** — the compiler will list every
   missing field.

7. **Add the locale to the language picker** in
   `crates/sui-id-web/src/pages/me_security/language.rs` (radio button)
   and any server-settings select that lists supported locales.

## Translation guidelines

- Prefer concise labels — most strings appear in table headers or buttons.
- Error messages should be specific but not reveal internal details.
- Use the locale's standard date conventions; the per-locale `*_fmt_date`
  and `*_fmt_relative` functions in your locale file handle date rendering.
- For `email_greeting_suffix`: in Japanese this is `""` (greeting suffix
  follows the name in "Alice さん"); check existing locales for the pattern.

## Running tests

```bash
cargo test -p sui-id-i18n
```

Tests assert that every locale in `Locale::ALL` has non-empty required
fields. Per-locale formatters are tested with representative date
assertions in each locale file's `#[cfg(test)] mod tests` block.

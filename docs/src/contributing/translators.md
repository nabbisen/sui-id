# Adding or updating translations

sui-id uses a compile-time exhaustive translation system in `crates/sui-id-i18n`.
Missing translations are **compile errors**, not runtime warnings.

## How the system works

Every translatable string is a field on `Strings` (in `strings.rs`).
Each supported locale has a `static STRINGS_*` constant that provides
every field. Adding a field to `Strings` makes all locale files fail
to compile until they supply the new value.

## Adding a new locale

1. **Add the variant** to `Locale` in `crates/sui-id-i18n/src/lib.rs`:

   ```rust
   pub enum Locale {
       Ja,
       En,
       Zh,
       Ko,  // ← new
   }
   ```

2. **Add the BCP-47 tag** in `Locale::tag()`:

   ```rust
   Self::Ko => "ko",
   ```

3. **Add the native name** in `Locale::native_name()`:

   ```rust
   Self::Ko => "한국어",
   ```

4. **Add the direction** in `Locale::direction()` (use `"rtl"` for Arabic, Hebrew):

   ```rust
   Self::Ko => "ltr",
   ```

5. **Add to `Locale::ALL`**:

   ```rust
   pub const ALL: &'static [Locale] = &[Locale::Ja, Locale::En, Locale::Zh, Locale::Ko];
   ```

6. **Add parse support** in `Locale::parse()`:

   ```rust
   "ko" => Some(Locale::Ko),
   ```

7. **Add strings** in `Locale::strings()` and `Locale::formatters()`:

   ```rust
   Self::Ko => &STRINGS_KO,
   // ...
   Self::Ko => &FORMATTERS_KO,
   ```

8. **Create translation files**:
   - `crates/sui-id-i18n/src/ko.rs` — copy `en.rs` as a starting point
   - Add `pub static FORMATTERS_KO` to `crates/sui-id-i18n/src/formatters.rs`

9. **Run `cargo check -p sui-id-i18n`** — the compiler will list every
   missing field.

## Updating an existing translation

Open the relevant locale file (`ja.rs`, `en.rs`, or `zh.rs`) and edit
the field value. Run `cargo test -p sui-id-i18n` to verify.

## Translation guidelines

- Prefer concise labels — most strings appear in table headers or button text.
- Error messages should be specific but not reveal internal details.
- Use the locale's standard date conventions; the `Formatters` struct
  handles date rendering — most translators only need to update string labels.
- For the `email_greeting_suffix` field: in Japanese this is `""` (empty, 
  because greeting suffix comes after the name in "Alice さん"); in English
  it is `""` too (English greetings are handled differently). Check the
  existing locales for the pattern.

## Running tests

```bash
cargo test -p sui-id-i18n
```

The test suite asserts that every locale in `Locale::ALL` has non-empty
values for a selection of required fields. Formatters are tested with
snapshot assertions on representative dates.

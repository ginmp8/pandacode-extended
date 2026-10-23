//! Focused behavioral checks for the security dependency changes.

#[cfg(test)]
mod tests {
    use glib::variant::ToVariant;
    use std::io::{Cursor, Read, Write};

    fn strings() -> glib::Variant {
        std::hint::black_box(vec!["first", "βeta", "third", "last"]).to_variant()
    }

    #[test]
    fn glib_strings_survive_optimized_forward_iteration() {
        let value = strings();
        let values: Vec<_> = value.array_iter_str().unwrap().collect();
        assert_eq!(values, ["first", "βeta", "third", "last"]);
    }

    #[test]
    fn glib_strings_survive_optimized_reverse_iteration() {
        let value = strings();
        let values: Vec<_> = value.array_iter_str().unwrap().rev().collect();
        assert_eq!(values, ["last", "third", "βeta", "first"]);
    }

    #[test]
    fn glib_strings_survive_indexed_and_last_access() {
        let value = strings();
        assert_eq!(value.array_iter_str().unwrap().nth(1), Some("βeta"));
        assert_eq!(value.array_iter_str().unwrap().nth_back(1), Some("third"));
        assert_eq!(value.array_iter_str().unwrap().last(), Some("last"));
    }

    #[test]
    fn glib_string_iterator_exhausts_without_reusing_an_out_pointer() {
        let value = strings();
        let mut iter = value.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some("first"));
        assert_eq!(iter.next_back(), Some("last"));
        assert_eq!(iter.next(), Some("βeta"));
        assert_eq!(iter.next_back(), Some("third"));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        let empty = Vec::<String>::new().to_variant();
        assert_eq!(empty.array_iter_str().unwrap().next(), None);
    }

    #[test]
    fn zip_supports_the_existing_bounded_vsix_reading_apis() {
        let json = r##"{"colors":{"editor.background":"#abcdef"}}"##;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut bytes);
            writer.start_file("extension/theme.json", zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)).unwrap();
            writer.write_all(json.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.into_inner())).unwrap();
        assert_eq!(archive.len(), 1);
        let mut file = archive.by_name("extension/theme.json").unwrap();
        assert_eq!(file.size(), json.len() as u64);
        let mut text = String::new();
        file.by_ref().take(1024).read_to_string(&mut text).unwrap();
        assert_eq!(text, json);
    }

    #[test]
    fn zip_rejects_a_non_archive_before_reading_a_theme() {
        assert!(zip::ZipArchive::new(Cursor::new(b"not a zip archive")).is_err());
    }
}

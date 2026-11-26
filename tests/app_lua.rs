use ngs_olezhek::testing::LuaTests;

#[test]
fn example_app_pages() {
    LuaTests::new("examples/hello/lua").run_dir("examples/hello/tests");
}

#[test]
fn failures_are_collected_with_their_location() {
    let dir = std::env::temp_dir().join(format!("ngs-lua-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("broken_test.lua");
    std::fs::write(
        &file,
        r#"
        test("passes", function(t) t.Eq({ a = 1 }, { a = 1 }) end)
        test("wrong value", function(t) t.Eq(1 + 1, 3, "sum") end)
        test("missing panel", function(t)
            t.Go("/nowhere")
            t.Get({ Class = "Button", Text = "Nope" })
        end)
        test("unanswered errors fail", function(t) ErrorNoHalt("left behind") end)
        test("taken errors pass", function(t)
            ErrorNoHalt("expected")
            t.Eq(#t.Errors(), 1)
        end)
        "#,
    )
    .unwrap();
    let failures = LuaTests::new("examples/hello/lua").check_files(&[file]);
    let _ = std::fs::remove_dir_all(&dir);
    let names: Vec<_> = failures.iter().map(|f| f.test.as_str()).collect();
    assert_eq!(names, ["wrong value", "missing panel", "unanswered errors fail"]);
    assert!(failures[0].message.contains("sum: expected 3, got 2") && failures[0].message.contains("broken_test.lua:3"), "{}", failures[0].message);
    assert!(failures[1].message.contains("no panel matches") && failures[1].message.contains("404"), "{}", failures[1].message);
    assert!(failures[2].message.contains("left behind"));
}

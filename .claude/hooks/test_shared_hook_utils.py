import unittest

from test_helpers import load_hook_module

shared = load_hook_module("_shared")


class SharedHookUtilsTest(unittest.TestCase):
    def test_flatten_text_handles_nested_content_blocks(self) -> None:
        value = [
            {"type": "text", "text": "first line"},
            {"type": "tool_result", "content": [{"text": "second line"}]},
            {"message": "third line"},
        ]

        flattened = shared.flatten_text(value, shared.RESPONSE_TEXT_KEYS)

        self.assertEqual(flattened, "first line\nsecond line\nthird line")

    def test_tool_response_text_includes_result_blocks(self) -> None:
        response = {
            "stdout": "stdout line",
            "result": [{"type": "text", "text": "result line"}],
        }

        flattened = shared.tool_response_text(response)

        self.assertEqual(flattened, "stdout line\nresult line")

    def test_tool_input_text_handles_structured_blocks(self) -> None:
        tool_input_data = {
            "content": [{"type": "text", "text": "pub trait Repo {}"}],
            "new_string": [{"message": "async fn run() {}"}],
        }

        flattened = shared.tool_input_text(tool_input_data, "content", "new_string")

        self.assertEqual(flattened, "pub trait Repo {}\nasync fn run() {}")

    def test_flatten_text_ignores_metadata_only_strings(self) -> None:
        value = {
            "type": "tool_result",
            "role": "assistant",
            "id": "msg_123",
            "payload": [
                {"kind": "segment", "text": "useful line"},
                {"meta": "ignore me", "nested": {"message": "second line"}},
            ],
        }

        flattened = shared.flatten_text(value, shared.RESPONSE_TEXT_KEYS)

        self.assertEqual(flattened, "useful line\nsecond line")

    def test_input_and_response_flattening_use_different_key_sets(self) -> None:
        value = {
            "stdout": "build log",
            "message": "shared message",
        }

        input_flattened = shared.flatten_text(value, shared.INPUT_TEXT_KEYS)
        response_flattened = shared.flatten_text(value, shared.RESPONSE_TEXT_KEYS)

        self.assertEqual(input_flattened, "shared message")
        self.assertEqual(response_flattened, "build log\nshared message")


if __name__ == "__main__":
    unittest.main()

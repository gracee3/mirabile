import unittest

from workflow_assertions import approximately, circularly_approximately, normalize_parity


class WorkflowAssertionsTest(unittest.TestCase):
    def test_approximate_and_circular_boundaries(self) -> None:
        approximately(2.28193, 2.28, 0.01)
        circularly_approximately(359.99, 0.01, 0.03)
        with self.assertRaises(AssertionError):
            circularly_approximately(359.0, 1.0, 1.5)

    def test_normalization_removes_generated_identity_and_trace_noise(self) -> None:
        left = {"projection": 4, "resource_id": "a", "points": [{"id": "sun", "x": 1}]}
        right = {"projection": 99, "resource_id": "b", "points": [{"x": 1, "id": "sun"}]}
        self.assertEqual(normalize_parity(left), normalize_parity(right))


if __name__ == "__main__":
    unittest.main()

module.exports = {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      ["feat", "fix", "perf", "revert", "refactor", "docs", "style", "test", "build", "ci", "chore"],
    ],
    "scope-enum": [
      1, // warn only
      "always",
      ["nodejs", "rust", "schema", "fixtures", "ci", "docs"],
    ],
    "subject-case": [0],
    "body-max-line-length": [0], // semantic-release changelog commits have long bodies
  },
};

/** Tailwind scans the Maud templates (class strings live in .rs files). */
module.exports = {
  content: ["./crates/hub/src/**/*.rs"],
  theme: {
    extend: {
      colors: {
        ink: "#0b1020",
        panel: "#121a2e",
        line: "#1f2a44",
      },
    },
  },
  plugins: [],
};

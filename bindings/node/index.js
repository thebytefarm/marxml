// Placeholder for the marxml npm name reservation.
// The working API lands in 0.1.0 via napi-rs prebuilt binaries.
// See https://github.com/thebytefarm/marxml.

const PLACEHOLDER_MESSAGE =
  "marxml is still in pre-release. The working API lands in 0.1.0. " +
  "See https://github.com/thebytefarm/marxml for status.";

function notYet() {
  throw new Error(PLACEHOLDER_MESSAGE);
}

module.exports = {
  parse: notYet,
  validate: notYet,
  __placeholder: true,
};

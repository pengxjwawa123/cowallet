class ActionResult {
  final bool success;
  final String message;
  final Map<String, String> data;

  const ActionResult({
    required this.success,
    required this.message,
    this.data = const {},
  });

  const ActionResult.ok(this.message, {this.data = const {}}) : success = true;

  const ActionResult.fail(this.message, {this.data = const {}})
      : success = false;
}

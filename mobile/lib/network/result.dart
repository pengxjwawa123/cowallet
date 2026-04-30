sealed class Result<T> {
  const Result();

  factory Result.success(T data) = Success<T>;
  factory Result.error(String message, int code) = Error<T>;

  // 快捷判断
  bool get isSuccess => this is Success<T>;
  bool get isError => this is Error<T>;

  // 快捷获取数据
  T? get data => this is Success<T> ? (this as Success<T>).data : null;
  String? get errorMessage => this is Error<T> ? (this as Error<T>).message : null;
  int? get errorCode => this is Error<T> ? (this as Error<T>).code : null;
}

class Success<T> extends Result<T> {
  final T data;
  const Success(this.data);
}

class Error<T> extends Result<T> {
  final String message;
  final int code;
  const Error(this.message, this.code);
}

import 'package:dio/dio.dart';
import 'package:pretty_dio_logger/pretty_dio_logger.dart';
import '../config/api_config.dart';
import '../utils/secure_storage.dart';
import 'result.dart';

class DioClient {
  static Dio? _instance;

  static Dio get instance {
    if (_instance == null) {
      _initDio();
    }
    return _instance!;
  }

  static void _initDio() {
    BaseOptions options = BaseOptions(
      baseUrl: ApiConfig.apiBaseUrl,
      connectTimeout: Duration(seconds: ApiConfig.connectTimeout),
      receiveTimeout: Duration(seconds: ApiConfig.receiveTimeout),
      headers: {
        "Content-Type": "application/json",
        "Accept": "application/json",
      },
    );

    _instance = Dio(options);

    // 添加拦截器
    _instance!.interceptors.addAll([
      // Token自动添加拦截器
      InterceptorsWrapper(
        onRequest: (options, handler) async {
          // 从安全存储拿token，自动加到请求头
          String? token = await SecureStorage.getToken();
          if (token != null && token.isNotEmpty) {
            options.headers["Authorization"] = "Bearer $token";
            print("✅ Token added to Authorization header");
          } else {
            print("⚠️  Token is null or empty");
          }
          return handler.next(options);
        },
        onResponse: (response, handler) {
          return handler.next(response);
        },
        onError: (DioException e, handler) async {
          // 401未授权，token过期，自动清除登录信息
          if (e.response?.statusCode == 401) {
            print("⚠️  401 Unauthorized - clearing token");
            await SecureStorage.clearAll();
            // TODO: 这里接入全局路由后，自动跳转到登录页
            // navigatorKey.currentState?.pushReplacementNamed('/login');
          }
          return handler.next(e);
        },
      ),
      // 日志拦截器，开发环境开启，生产环境关闭
      if (!const bool.fromEnvironment('dart.vm.product'))
        PrettyDioLogger(
          requestHeader: true,
          requestBody: true,
          responseHeader: true,
          responseBody: true,
          error: true,
          maxWidth: 120,
        ),
    ]);
  }

  // 统一请求方法，返回Result封装
  static Future<Result<T>> request<T>(
    String path, {
    String method = "GET",
    Map<String, dynamic>? params,
    dynamic data,
    Options? options,
    CancelToken? cancelToken,
  }) async {
    try {
      Options requestOptions = options ?? Options();
      requestOptions.method = method;

      Response response = await instance.request(
        path,
        queryParameters: params,
        data: data,
        options: requestOptions,
        cancelToken: cancelToken,
      );

      // 响应处理 - 根据你的后端实际返回格式调整
      if (response.statusCode == 200 || response.statusCode == 201) {
        // 如果后端直接返回数据，没有外层包装
        return Result.success(response.data as T);
        // 如果后端有标准包装格式：{ "code": 0, "msg": "success", "data": {} }
        // 取消上面的注释，用下面的逻辑：
        // if (response.data["code"] == 0) {
        //   return Result.success(response.data["data"] as T);
        // } else {
        //   return Result.error(
        //     response.data["msg"] ?? "请求失败",
        //     response.data["code"] ?? -1,
        //   );
        // }
      } else {
        return Result.error(
          "请求失败，状态码：${response.statusCode}",
          response.statusCode ?? -1,
        );
      }
    } on DioException catch (e) {
      String errorMsg = _handleError(e);
      return Result.error(errorMsg, e.response?.statusCode ?? -1);
    } catch (e) {
      return Result.error("未知错误：${e.toString()}", -1);
    }
  }

  // 快捷请求方法
  static Future<Result<T>> get<T>(
    String path, {
    Map<String, dynamic>? params,
    Options? options,
    CancelToken? cancelToken,
  }) async {
    return request<T>(
      path,
      method: "GET",
      params: params,
      options: options,
      cancelToken: cancelToken,
    );
  }

  static Future<Result<T>> post<T>(
    String path, {
    dynamic data,
    Map<String, dynamic>? params,
    Options? options,
    CancelToken? cancelToken,
  }) async {
    return request<T>(
      path,
      method: "POST",
      data: data,
      params: params,
      options: options,
      cancelToken: cancelToken,
    );
  }

  static Future<Result<T>> put<T>(
    String path, {
    dynamic data,
    Map<String, dynamic>? params,
    Options? options,
    CancelToken? cancelToken,
  }) async {
    return request<T>(
      path,
      method: "PUT",
      data: data,
      params: params,
      options: options,
      cancelToken: cancelToken,
    );
  }

  static Future<Result<T>> delete<T>(
    String path, {
    dynamic data,
    Map<String, dynamic>? params,
    Options? options,
    CancelToken? cancelToken,
  }) async {
    return request<T>(
      path,
      method: "DELETE",
      data: data,
      params: params,
      options: options,
      cancelToken: cancelToken,
    );
  }

  // 错误处理
  static String _handleError(DioException e) {
    switch (e.type) {
      case DioExceptionType.connectionTimeout:
        return "连接超时，请检查网络";
      case DioExceptionType.sendTimeout:
        return "请求发送超时";
      case DioExceptionType.receiveTimeout:
        return "响应超时，请稍后重试";
      case DioExceptionType.badResponse:
        int? statusCode = e.response?.statusCode;
        if (statusCode == 400) return "请求参数错误";
        if (statusCode == 401) return "登录已过期，请重新登录";
        if (statusCode == 403) return "没有权限访问";
        if (statusCode == 404) return "请求的资源不存在";
        if (statusCode == 500) return "服务器内部错误";
        return "服务器错误，状态码：$statusCode";
      case DioExceptionType.cancel:
        return "请求已取消";
      case DioExceptionType.connectionError:
        return "网络连接失败，请检查网络设置";
      default:
        return "未知网络错误，请稍后重试";
    }
  }
}

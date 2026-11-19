namespace KuroShell.Core;

public static class ApiAuthRetry
{
    public static bool ShouldRereadTokenOnce(int priorAttempts, int? httpStatus, bool pipeDenied) =>
        priorAttempts == 0 && (httpStatus == 401 || pipeDenied);
}

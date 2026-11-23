using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using WinRT;

namespace KuroShell;

/// Single-instance entry: a second Start Menu launch focuses the running shell.
public static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ComWrappersSupport.InitializeComWrappers();
        if (DecideRedirection())
        {
            return;
        }

        Application.Start(p =>
        {
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }

    private static bool DecideRedirection()
    {
        var instance = AppInstance.FindOrRegisterForKey("KuroShell.v1");
        if (!instance.IsCurrent)
        {
            var args = AppInstance.GetCurrent().GetActivatedEventArgs();
            RedirectAndWait(instance, args);
            return true;
        }

        instance.Activated += OnActivated;
        return false;
    }

    private static void RedirectAndWait(AppInstance target, AppActivationArguments args)
    {
        var done = new ManualResetEventSlim(false);
        _ = Task.Run(async () =>
        {
            try
            {
                await target.RedirectActivationToAsync(args);
            }
            finally
            {
                done.Set();
            }
        });
        done.Wait();
    }

    private static void OnActivated(object? sender, AppActivationArguments e)
    {
        if (Application.Current is not App app)
        {
            return;
        }

        var queue = App.UiQueue ?? App.Home?.DispatcherQueue ?? App.Hud?.DispatcherQueue;
        if (queue is null)
        {
            return;
        }

        _ = queue.TryEnqueue(() => _ = app.OpenHome());
    }
}

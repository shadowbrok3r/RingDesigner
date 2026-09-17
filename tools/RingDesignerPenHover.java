// Emulator probe: inject the same stylus hover events as an Android digitizer.
// Compile against android.jar, dex with d8, run as adb shell via app_process.
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.MotionEvent;
import java.lang.reflect.Method;

public final class RingDesignerPenHover {
    public static void main(String[] args) throws Exception {
        float x=Float.parseFloat(args[0]), y=Float.parseFloat(args[1]);
        long duration=Long.parseLong(args[2]);
        if(duration<700 || duration>10000) throw new IllegalArgumentException("700–10000 ms");
        Class<?> type=Class.forName("android.hardware.input.InputManagerGlobal");
        Object manager=type.getMethod("getInstance").invoke(null);
        Method inject=type.getMethod("injectInputEvent",InputEvent.class,int.class);
        MotionEvent.PointerProperties p=new MotionEvent.PointerProperties();
        p.id=0; p.toolType=MotionEvent.TOOL_TYPE_STYLUS;
        MotionEvent.PointerCoords c=new MotionEvent.PointerCoords();
        c.x=x; c.y=y; c.pressure=0; c.size=0;
        c.setAxisValue(MotionEvent.AXIS_DISTANCE,0.25f);
        long start=SystemClock.uptimeMillis();
        for(int step=0;;step++) {
            long now=SystemClock.uptimeMillis();
            int action=step==0?MotionEvent.ACTION_HOVER_ENTER:now-start>=duration?MotionEvent.ACTION_HOVER_EXIT:MotionEvent.ACTION_HOVER_MOVE;
            MotionEvent e=MotionEvent.obtain(start,now,action,1,new MotionEvent.PointerProperties[]{p},new MotionEvent.PointerCoords[]{c},0,0,1,1,0,0,InputDevice.SOURCE_STYLUS,0);
            if(!Boolean.TRUE.equals(inject.invoke(manager,e,2))) throw new IllegalStateException("hover injection failed");
            e.recycle();
            if(action==MotionEvent.ACTION_HOVER_EXIT) break;
            SystemClock.sleep(50);
        }
    }
}

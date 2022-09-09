use std::cell::Cell;
use std::rc::Rc;

use windows::core::{implement, Error, InParam, Result, GUID, HRESULT};
use windows::Win32::Foundation::{HANDLE_PTR, POINT};

use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::TabletPC::{
    IInkTablet, IRealTimeStylus, IStylusAsyncPlugin, IStylusAsyncPlugin_Impl, IStylusPlugin,
    IStylusPlugin_Impl, RTSDI_AllData, RealTimeStylus, RealTimeStylusDataInterest, StylusInfo,
    SYSTEM_EVENT_DATA,
};

use crate::{EasyTabError, EasyTabOptions, EasyTabResult, EasyTablet, __InnerTablet};

// ///
// #[derive(Default, Clone, Copy, Debug)]
// pub enum WinPropertyMetricUnit {
//     #[default]
//     Default = 0,
//     Inches,
//     Centimeters,
//     Degrees,
//     Radians,
//     Seconds,
//     Pounds,
//     Grams,

//     #[doc(hidden)]
//     // final element used to get the number of variants in the enum
//     __Final,
// }

// impl From<TabletPropertyMetricUnit> for WinPropertyMetricUnit {
//     fn from(tpm: TabletPropertyMetricUnit) -> Self {
//         assert!(tpm.0 < WinPropertyMetricUnit::__Final as i32);
//         // Safety:
//         // Check performed above to make sure the inner value is not larger than the length of the enum.
//         unsafe { std::mem::transmute(tpm.0 as i8) }
//     }
// }

// ///
// #[derive(Default, Clone, Copy, Debug)]
// pub struct Property {
//     min: i32,
//     max: i32,

//     units: TabletPropertyMetricUnit,
//     resolution: f32,
// }

// ///
// pub struct WinTab {}

//
#[derive(Default)]
pub enum WinTabletIndex {
    #[default]
    Default,
    Index(i32),
}

// ///
// #[repr(u64)]
// pub enum EasyTabProperty {
//     X = TabletPC::GUID_PACKETPROPERTY_GUID_X.to_u128() as u64,
//     Y,
//     Z,

//     PacketStatus,
//     TimerTick,
//     SerialNumber,

//     NormalPressure,
//     TangentPressure,
//     ButtonPressure,

//     XTiltOrientation,
//     YTiltOrientation,
//     AzimuthOrientation,
//     TwistOrientation,

//     PitchRotation,
//     RollRotation,
//     YawRotation,

//     Width,
//     Height,

//     FingerContantConfidence,
//     ContactId,
// }

// function used to map a windows errors to an easytab error
const ERROR_FN: fn(Error) -> EasyTabError = |e| EasyTabError::WinError(e.message());

impl EasyTablet {
    /// Initialises a tablet.
    ///
    /// ## Arguments
    ///
    /// - `hwnd`: `HANDLE_PTR` - a handle to a window to bind the tablet to.
    ///
    /// <br>
    ///
    /// **Note**: This functions requires that [`CoInitializeEx`](https://docs.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex) has previously been called.
    /// - Refer to [`init_options`] for more info.
    pub fn init(hwnd: HANDLE_PTR) -> EasyTabResult<Self> {
        EasyTablet::init_options(hwnd, EasyTabOptions::default())
    }

    /// Initialises a tablet with the given options.
    ///
    /// ## Arguments
    ///
    /// - `hwnd`: `HANDLE_PTR`&emsp;&emsp;- a handle to a window to bind the tablet to.
    /// - `opts`: `WinTabOptions` - the initialisation options for the tablet.
    ///
    /// <br>
    ///
    /// **Note**: This functions requires that [`CoInitializeEx`](https://docs.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex) has previously been called.
    ///
    /// ```
    /// // before calling `init_options`
    /// unsafe {
    ///     CoInitializeEx(
    ///         std::ptr::null(),
    ///         COINIT_APARTMENTTHREADED,
    ///     )
    ///     .expect("failed to initalise COM");
    /// }
    /// //...
    /// let tablet = EasyTablet::init_options(...)?;
    /// ```
    pub fn init_options(hwnd: HANDLE_PTR, opts: EasyTabOptions) -> EasyTabResult<Self> {
        // create a real time stylus
        let stylus: IRealTimeStylus = unsafe {
            CoCreateInstance(&RealTimeStylus, InParam::null(), CLSCTX_INPROC_SERVER)
                .map_err(ERROR_FN)?
        };

        // bind the stylus to the current window
        unsafe { stylus.SetHWND(hwnd).map_err(ERROR_FN)? };

        let slf = Self(Rc::new(__InnerTablet {
            stylus,
            opts,

            active: Cell::default(),
            x: Cell::default(),
            y: Cell::default(),
            pressure: Cell::default(),
        }));

        // pass a reference of ourselves into the handler so it can call the `handle_event` fn
        let ash: IStylusAsyncPlugin = AsyncStylusHandler(Rc::clone(&slf.0)).into();

        // add the handler to the stylus
        unsafe {
            slf.stylus
                .AddStylusAsyncPlugin(
                    slf.stylus.GetStylusAsyncPluginCount().map_err(ERROR_FN)?,
                    &ash,
                )
                .map_err(ERROR_FN)?
        };

        Ok(slf)
    }

    /// Enables the tablet.
    pub fn enable(&self) -> EasyTabResult<()> {
        unsafe { self.stylus.SetEnabled(true).map_err(ERROR_FN)? };

        Ok(())
    }

    /// Disables the tablet.
    pub fn disable(&self) -> EasyTabResult<()> {
        unsafe { self.stylus.SetEnabled(false).map_err(ERROR_FN)? };

        Ok(())
    }

    /// Returns whether a finger or stylus is activating the digitiser.
    pub fn active(&self) -> bool {
        self.active.get()
    }

    /// Returns the x position where the finger or stylus is making contact with the digitiser.
    pub fn x(&self) -> u32 {
        self.x.get()
    }

    /// Returns the y position where the finger or stylus is making contact with the digitiser.
    pub fn y(&self) -> u32 {
        self.y.get()
    }

    /// Returns the pressure of the finger or stylus on the digitiser.
    pub fn pressure(&self) -> f32 {
        self.pressure.get()
    }
}

impl __InnerTablet {
    // handles a stylus event
    fn handle_event(&self, event: WinTabEvent) -> Result<()> {
        match event {
            WinTabEvent::StylusDown => self.active.set(true),
            _ => todo!(),
        }

        Ok(())
    }
}

pub enum WinTabEvent {
    StylusDown,
    StylusUp,
}

// the plugin added to the real time stylus to allow getting real time events from the stylus (asynchronously)
#[implement(IStylusAsyncPlugin)]
struct AsyncStylusHandler(Rc<__InnerTablet>);

impl IStylusPlugin_Impl for AsyncStylusHandler {
    fn RealTimeStylusEnabled(
        &self,
        _: &Option<IRealTimeStylus>,
        _: u32,
        _: *const u32,
    ) -> Result<()> {
        Ok(())
    }

    fn RealTimeStylusDisabled(
        &self,
        _: &Option<IRealTimeStylus>,
        _: u32,
        _: *const u32,
    ) -> Result<()> {
        Ok(())
    }

    fn StylusInRange(&self, _: &Option<IRealTimeStylus>, _: u32, _: u32) -> Result<()> {
        Ok(())
    }

    fn StylusOutOfRange(&self, _: &Option<IRealTimeStylus>, _: u32, _: u32) -> Result<()> {
        Ok(())
    }

    fn StylusDown(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        _: *const StylusInfo,
        _: u32,
        _: *const i32,
        _: *mut *mut i32,
    ) -> Result<()> {
        debug_assert!(unsafe { pirtssrc.as_ref().unwrap_unchecked() } == &self.0.as_ref().stylus);

        self.0.as_ref().handle_event(WinTabEvent::StylusDown)
    }

    fn StylusUp(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        pstylusinfo: *const StylusInfo,
        cpropcountperpkt: u32,
        ppacket: *const i32,
        ppinoutpkt: *mut *mut i32,
    ) -> Result<()> {
        // debug_assert!(unsafe { pirtssrc.as_ref().unwrap_unchecked() == &(*self.0).stylus });

        println!("StylusUp");

        // unsafe { (&*self.0).handle_event(WinTabEvent::StylusUp) }
        Ok(())
    }

    fn StylusButtonDown(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        sid: u32,
        pguidstylusbutton: *const GUID,
        pstyluspos: *mut POINT,
    ) -> Result<()> {
        println!("StylusButtonDown");
        Ok(())
    }

    fn StylusButtonUp(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        sid: u32,
        pguidstylusbutton: *const GUID,
        pstyluspos: *mut POINT,
    ) -> Result<()> {
        println!("StylusButtonUp");
        Ok(())
    }

    fn InAirPackets(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        pstylusinfo: *const StylusInfo,
        cpktcount: u32,
        cpktbufflength: u32,
        ppackets: *const i32,
        pcinoutpkts: *mut u32,
        ppinoutpkts: *mut *mut i32,
    ) -> Result<()> {
        //println!("InAirPackets");
        Ok(())
    }

    fn Packets(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        pstylusinfo: *const StylusInfo,
        cpktcount: u32,
        cpktbufflength: u32,
        ppackets: *const i32,
        pcinoutpkts: *mut u32,
        ppinoutpkts: *mut *mut i32,
    ) -> Result<()> {
        println!("Packets");
        Ok(())
    }

    fn CustomStylusDataAdded(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        pguidid: *const GUID,
        cbdata: u32,
        pbdata: *const u8,
    ) -> Result<()> {
        println!("CustomStylusDataAdded");
        Ok(())
    }

    fn SystemEvent(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        tcid: u32,
        sid: u32,
        event: u16,
        eventdata: &SYSTEM_EVENT_DATA,
    ) -> Result<()> {
        println!("SystemEvent");
        Ok(())
    }

    fn TabletAdded(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        pitablet: &Option<IInkTablet>,
    ) -> Result<()> {
        println!("TabletAdded");
        Ok(())
    }

    fn TabletRemoved(&self, pirtssrc: &Option<IRealTimeStylus>, itabletindex: i32) -> Result<()> {
        println!("TabletRemoved");
        Ok(())
    }

    fn Error(
        &self,
        pirtssrc: &Option<IRealTimeStylus>,
        piplugin: &Option<IStylusPlugin>,
        datainterest: RealTimeStylusDataInterest,
        hrerrorcode: HRESULT,
        lptrkey: *mut isize,
    ) -> Result<()> {
        println!("Error");
        Ok(())
    }

    fn UpdateMapping(&self, pirtssrc: &Option<IRealTimeStylus>) -> Result<()> {
        println!("UpdateMapping");
        Ok(())
    }

    fn DataInterest(&self) -> Result<RealTimeStylusDataInterest> {
        // collect all data on stylus events
        Ok(RTSDI_AllData)
    }
}

impl IStylusAsyncPlugin_Impl for AsyncStylusHandler {}

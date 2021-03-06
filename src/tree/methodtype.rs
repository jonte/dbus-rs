// Methods and method types. Glue to make stuff generic over MFn, MFnMut and MSync

use std::fmt;
use {ErrorName, Message};
use arg::{Iter, IterAppend, TypeMismatchError};
use std::marker::PhantomData;
use super::{Method, Interface, Property, ObjectPath, Tree};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
/// A D-Bus Method Error.
pub struct MethodErr(ErrorName<'static>, String);

impl MethodErr {
    /// Create an Invalid Args MethodErr.
    pub fn invalid_arg<T: fmt::Debug>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a)).into()
    }
    /// Create a MethodErr that there are not enough arguments given.
    pub fn no_arg() -> MethodErr {
        ("org.freedesktop.DBus.Error.InvalidArgs", "Not enough arguments").into()
    }
    /// Create a MethodErr that the method failed in the way specified.
    pub fn failed<T: fmt::Display>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.Failed", a.to_string()).into()
    }
    /// Create a MethodErr that the Interface was unknown.
    pub fn no_interface<T: fmt::Display>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", a)).into()
    }
    /// Create a MethodErr that the Method was unknown.
    pub fn no_method<T: fmt::Display>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownMethod", format!("Unknown method {}", a)).into()
    }
    /// Create a MethodErr that the Property was unknown.
    pub fn no_property<T: fmt::Display>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownProperty", format!("Unknown property {}", a)).into()
    }
    /// Create a MethodErr that the Property was read-only.
    pub fn ro_property<T: fmt::Display>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.PropertyReadOnly", format!("Property {} is read only", a)).into()
    }

    pub fn errorname(&self) -> &ErrorName<'static> { &self.0 }
    pub fn description(&self) -> &str { &self.1 }
}

impl From<TypeMismatchError> for MethodErr {
    fn from(t: TypeMismatchError) -> MethodErr { ("org.freedesktop.DBus.Error.Failed", format!("{}", t)).into() }
}

impl<T: Into<ErrorName<'static>>, M: Into<String>> From<(T, M)> for MethodErr {
    fn from((t, m): (T, M)) -> MethodErr { MethodErr(t.into(), m.into()) }
}

/// Result containing the Messages returned from the Method, or a MethodErr.
pub type MethodResult = Result<Vec<Message>, MethodErr>;

/// Associated data for different objects in a tree.
///
/// These currently require a debug bound, due to https://github.com/rust-lang/rust/issues/31518
pub trait DataType: Sized + Default {
    type ObjectPath: fmt::Debug;
    type Property: fmt::Debug;
    type Interface: fmt::Debug + Default;
    type Method: fmt::Debug + Default;
    type Signal: fmt::Debug;
}

/// No associated data for the tree.
impl DataType for () {
    type ObjectPath = ();
    type Interface = ();
    type Property = ();
    type Method = ();
    type Signal = ();
}

/// A helper trait used internally to make the tree generic over MTFn, MTFnMut and MTSync.
///
/// You should not need to call these methods directly, it's primarily for internal use.
pub trait MethodType<D: DataType>: Sized + Default {
    type Method: ?Sized;
    type GetProp: ?Sized;
    type SetProp: ?Sized;

    fn call_getprop(&Self::GetProp, &mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    fn call_setprop(&Self::SetProp, &mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    fn call_method(&Self::Method, &MethodInfo<Self, D>) -> MethodResult;

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static;
}


/// An abstract type to represent Fn functions.
#[derive(Default, Debug, Copy, Clone)]
pub struct MTFn<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTFn<D> {
    type GetProp = Fn(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    type SetProp = Fn(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr>;
    type Method = Fn(&MethodInfo<Self, D>) -> MethodResult;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { p(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static { Box::new(h) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(h) }
}

/// An abstract type to represent FnMut functions.
#[derive(Default, Debug, Copy, Clone)]
pub struct MTFnMut<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTFnMut<D> {
    type GetProp = RefCell<FnMut(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr>>;
    type SetProp = RefCell<FnMut(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr>>;
    type Method = RefCell<FnMut(&MethodInfo<Self, D>) -> MethodResult>;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { (&mut *p.borrow_mut())(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { (&mut *p.borrow_mut())(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { (&mut *p.borrow_mut())(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static { Box::new(RefCell::new(h)) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(RefCell::new(h)) }

}

/// An abstract type to represent Fn + Send + Sync functions (that can be called from several threads in parallel).
#[derive(Default, Debug, Copy, Clone)]
pub struct MTSync<D=()>(PhantomData<*const D>);

impl<D: DataType> MethodType<D> for MTSync<D> {
    type GetProp = Fn(&mut IterAppend, &PropInfo<Self, D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    type SetProp = Fn(&mut Iter, &PropInfo<Self, D>) -> Result<(), MethodErr> + Send + Sync + 'static;
    type Method = Fn(&MethodInfo<Self, D>) -> MethodResult + Send + Sync + 'static;

    fn call_getprop(p: &Self::GetProp, i: &mut IterAppend, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_setprop(p: &Self::SetProp, i: &mut Iter, pinfo: &PropInfo<Self, D>)
        -> Result<(), MethodErr> { p(i, pinfo) }
    fn call_method(p: &Self::Method, minfo: &MethodInfo<Self, D>)
        -> MethodResult { p(minfo) }

    fn make_getprop<H>(h: H) -> Box<Self::GetProp>
    where H: Fn(&mut IterAppend, &PropInfo<Self,D>) -> Result<(), MethodErr> + Send + Sync + 'static  { Box::new(h) }
    fn make_method<H>(h: H) -> Box<Self::Method>
    where H: Fn(&MethodInfo<Self,D>) -> MethodResult + Send + Sync + 'static { Box::new(h) }
}



#[derive(Debug, Copy, Clone)]
/// Contains information about the incoming method call.
pub struct MethodInfo<'a, M: 'a + MethodType<D>, D: 'a + DataType> {
    pub msg: &'a Message,
    pub method: &'a Method<M, D>,
    pub iface: &'a Interface<M, D>,
    pub path: &'a ObjectPath<M, D>,
    pub tree: &'a Tree<M, D>,
}

impl<'a, M: 'a + MethodType<D>, D: 'a + DataType> MethodInfo<'a, M, D> {
    pub fn to_prop_info(&self, iface: &'a Interface<M, D>, prop: &'a Property<M, D>) -> PropInfo<'a, M, D> {
        PropInfo { msg: self.msg, method: self.method, iface: iface, prop: prop, path: self.path, tree: self.tree }
    }
}

#[derive(Debug, Copy, Clone)]
/// Contains information about the incoming property get/set request.
pub struct PropInfo<'a, M: 'a + MethodType<D>, D: 'a + DataType> {
    pub msg: &'a Message,
    pub method: &'a Method<M, D>,
    pub prop: &'a Property<M, D>,
    pub iface: &'a Interface<M, D>,
    pub path: &'a ObjectPath<M, D>,
    pub tree: &'a Tree<M, D>,
}

impl<'a, M: 'a + MethodType<D>, D: 'a + DataType> PropInfo<'a, M, D> {
    pub fn to_method_info(&self) -> MethodInfo<'a, M, D> {
        MethodInfo { msg: self.msg, method: self.method, iface: self.iface, path: self.path, tree: self.tree }
    }
}

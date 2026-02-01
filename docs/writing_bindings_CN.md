# 绑定 OpenCascade 的类和函数

OpenCascade 是一个巨大的 C++ 项目，其发展跨越了数十年。它包含了大量令人印象深刻的功能，尽管它们在某种程度上被隐藏在一个（在我看来）丑陋/吓人的API后面。

这个项目的一个目标是在 OpenCascade 的基础上提供一个对开发者更为友善的API，这样更多的人就可以用它来构建很酷的东西。

为此我们使用 [CXX](https://cxx.rs/)，由 dtolnay 制作。CXX 自身在某种程度上也是不太友好的，对于第一次使用它的人来说有时候很难搞清楚到底发生了什么，所以这份文档希望展示本项目 (译者注: opencascade-rs) 如何使用它去绑定 OpenCascade 并暴露有用的功能。

## 文件组织

### `build.rs`

在最底部，我们有 [occt-sys 包](../crates/occt-sys/)，它静态地构建没有任何修改的OpenCascade c++项目。

在此之上时 [opencascade-sys 包](../crates/opencascade-sys)，它有一个 [build.rs](../crates/opencascade-sys/build.rs)  。使用必要的 `cxx` 基础设施将 OpenCascade 的包装器和 C++ 绑定编译出来。

为了主题的简单，OpenCascade 库我们都是静态链接的，并且使用 `cargo:rustc-link-lib=static=LIB_NAME_HERE` 的 cargo 指令实现。

### C++/Rust 桥接文件

为了将 C++ 类型暴露到 Rust 中，我们需要编写一个在 cxx.rs 中称之为 “桥接”文件或“桥接”模块的东西，反之亦然。

在桥接模块的最顶上，我们可以定义一些对于 C++ 和 Rust 同时可见的类型，例如可以被表示为 `u32` 的简单的枚举。

除此之外的几乎所有其他东西都需要放到一个 `unsafe extern "C++" {}` 的代码块中。在这个代码块中，我们可以声明不透明的 C++ 类型以暴露给 Rust 环境。只使用简单的 `type SOME_CPP_TYPE_HERE;` 进行声明就足够了。`cxx` 之后将会检索包含的头文件并且确保这个类型在 C++ 代码中存在。仅有类型定义，你几乎什么都不能做，所以应该开始声明那些你想使用的在 C++ 代码中存在的函数。

在我们的代码中对于如何定义这些函数有一些规定：

* 在桥接文件中的任何位置不能返回一个裸露的或私有的C++类型。返回值必须是 一个不可变引用，或者是一个智能指针例如 `UniquePtr<T>` 或 `SharedPtr<T>`。

* 如果你在绑定一个成员函数（不是一个普通的函数），你必须使用 `self` 关键字作为这个函数的第一个参数。
    * 如果这个函数在 c++ 侧是 `const` 的（这个函数不会修改 `self`），那么 `fn do_something(self: &TheCPPTypeHere)` 就已经足够。
    * 如果这个函数不是 `const`，那么函数签名必须为 `fn do_something(self: Pin<&mut TheCPPTypeHere>)`。
    * 搞错这一点将会导致大量晦涩的 C++ 编译错误。

* 如果你在绑定一个普通的函数，那么不要使用特殊的 `self` 关键字作为任何参数的名称，应该正确地使用`&T`和`Pin<&mut T b>`。

* 据我所知，从 Rust 到 C++ 的泛型不能正常使用。如果你有一个例如 `Handle<Edge>` 的 C++ 类型，你将需要声明自己的 C++ 类型例如 `HandleEdge` 或者其他你喜欢的名称，然后将它作为完整模板名称的别名（例如：`typedef opencascade::handle<Edge> HandleEdge;`）

* 你可以使用 `#[cxx_name = "SomeCPPFunctionName"]` 去告诉 `cxx` 你想使用的 C++ 函数的真实名称，也可以使用 `#[rust_name = "some_rust_fn_name"]` 去控制你想暴露到 Rust 侧的函数的名称。如果你不用这些属性，那么导出的 Rust 函数名称将会和 C++ 函数名称完全相同。

#### 获取一个 `Pin<&mut T>`

如果你有一个 `some_var: UniquePtr<T>`，你可以通过 `some_var.pin_mut()` 得到一个 `Pin<&mut T>`。`some_var` 必须被声明为 `mut` 可变的才可以这样做。

#### `construct_unique`

为 C++ 构造函数生成绑定有一些 [困难](https://github.com/dtolnay/cxx/issues/280)，在 cxx 中没有很好的支持。所以，我们不能从函数中直接返回一个 `T` 类型变量，这个返回的变量应该被包装成一个如前文所述的引用或者智能指针。

所以在实践中，几乎每一个暴露在这个包中的构造函数都是返回 `UniquePtr<T>` 类型。为每个构造函数手动定义一个c++包装函数是很枯燥乏味的，所以通过[巧妙地使用模板](https://github.com/dtolnay/cxx/issues/280#issuecomment-1344153115)，我们可以定义一个c++函数，它遵循一定数量的参数，用这些参数调用 `T` 的构造函数，并返回这个 `T` 的 `UniquePtr`：

```c++
// Generic template constructor
template <typename T, typename... Args> std::unique_ptr<T> construct_unique(Args... args) {
  return std::unique_ptr<T>(new T(args...));
}
```

这是一个绑定 `BRepPrimAPI_MakeBox` 构造函数的例子，者个类可以构造一个盒子（物理上那种的盒子 Box，不是 Rust 语言里的那种 Box 泛型）

```rust
#[cxx_name = "construct_unique"]
pub fn BRepPrimAPI_MakeBox_ctor(
    point: &gp_Pnt,
    dx: f64,
    dy: f64,
    dz: f64,
) -> UniquePtr<BRepPrimAPI_MakeBox>;
```

通过这个声明，我们可以绑定一个 C++ 构造函数并且将它在 Rust 侧层命名成 `BRepPrimAPI_MakeBox_ctor`，并且不需要编写任何额外的 C++ 代码。

### wrapper.hxx

有些情况下 cxx 的自动绑定不能正确计算，例如你可以试图去访问一个类的静态成员，cxx 无法得知你试图去访问的继承父类的多态方法，或者没有遵守上述 `construct_unique` 模式的构造器或函数。

作为最后的手段，你可以定义你自己的 C++ 包装函数去将原始代码的类型与逻辑包装成你自己想要的形式。

例如：“BRepAdaptor_Curve” 类型有一个直接返回 `gp_Pnt` 的 `Value()` 函数。虽然 `gp_Pnt` 是一个用于访问 XYZ 坐标的非常简单的类，但是我们不能直接返回它，因为这违反了上述关于 cxx 的规定。

为了解决这个问题，我在 cxx 桥接模块中定义一个如下的 Rust 函数：

```rust
pub fn BRepAdaptor_Curve_value(curve: &BRepAdaptor_Curve, u: f64) -> UniquePtr<gp_Pnt>;
```

之后我往 `wrapper.hxx` 中添加一个同样名称的 C++ 函数：

```c++
inline std::unique_ptr<gp_Pnt> BRepAdaptor_Curve_value(const BRepAdaptor_Curve &curve, const Standard_Real U) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(curve.Value(U)));
}
```

这个问题可能也可以通过聪明地使用模板来解决，但我不是很确定。

## 例子：绑定 STEP 文件导入功能

为了给一个真实的 "教程" 展示如何从 OpenCascade 中展示绑定新功能，我将详细介绍向这个项目中添加 STEP 导入功能所需的工作。你也可以从 [这个PR](https://github.com/bschwind/opencascade-rs/pull/33) 中看到关于这个功能的所有修改。 

你也可以在 OpenCascade 原生代码 [这里](https://dev.opencascade.org/doc/overview/html/occt_user_guides__step.html)看到加载 STEP 文件的整体流程。

```c++
STEPControl_Reader reader;
reader.ReadFile("object.step");
reader.TransferRoots();
TopoDS_Shape shape = reader.OneShape();
```



### 第 1 步 - 在桥接模块中声明 C++ 类型与函数
首先我们让 cxx 桥接模块知道 STEPControl_Reader 类型，同时定义一个构造函数：

```rust
type STEPControl_Reader;

#[cxx_name = "construct_unique"]
pub fn STEPControl_Reader_ctor() -> UniquePtr<STEPControl_Reader>;
```

幸运的是，reader 构造函数不需要参数。但是下一步，从一个文件中读取，需要传递一个 String 参数表示文件名称，cxx 提供了一些用于在 Rust 和 C++ 字符串之间转换的基础设施，（译者注：出于演示目的）不过现在我们必须在 `wrapper.hxx` 中手动定义一个函数：

```rust
pub fn read_step(
    reader: Pin<&mut STEPControl_Reader>,
    filename: String,
) -> IFSelect_ReturnStatus;
```

该函数返回一个 `IFSelect_ReturnStatus`，所以我们还需要在桥接模块中声明这个类型：

```rust
type IFSelect_ReturnStatus;
```

这里最酷的部分是 `IFSelect_ReturnStatus` 仅仅是一个简单的枚举类型，所以我们其实可以在桥接模块 `ffi` 模块中声明一个相同名称的 Rust 枚举，不过是在 `unsafe extern "C++" {}` 代码块之外：

```rust
#[derive(Debug)]
#[repr(u32)]
pub enum IFSelect_ReturnStatus {
    IFSelect_RetVoid,
    IFSelect_RetDone,
    IFSelect_RetError,
    IFSelect_RetFail,
    IFSelect_RetStop,
}
```

这意味着我们可以直接返回 `IFSelect_ReturnStatus` 类型而不需要将它包装在一个引用或者一个智能指针中。

### 第 2 步 - 编写一个 C++ 包装代码

包装器函数通常是琐碎的，它们通常只有一个来自Rust的易于绑定的签名，然后实现任何需要的翻译逻辑。在本例中，返回的是一个声明过的枚举。

```c++
inline IFSelect_ReturnStatus read_step(STEPControl_Reader &reader, rust::String theFileName) {
  return reader.ReadFile(theFileName.c_str());
}
```

### 第 3 步 - 在 wrapper.hxx 中包含正确的文件

在我们的例子中，我们需要吧这个放到 `wrapper.hxx` 的最顶端

```c++
#include <STEPControl_Reader.hxx>
```

### 第 4 步 - 声明剩余的 C++ 函数

下一个需要被绑定的函数是 `reader.TransferRoots()`。它的 C++ 定义如下所示：

```c++
Standard_EXPORT Standard_Integer TransferRoots(const Message_ProgressRange& theProgress = Message_ProgressRange());
```

幸运的是我们可以直接将它绑定到 Rust：

```rust
pub fn TransferRoots(
    self: Pin<&mut STEPControl_Reader>,
    progress: &Message_ProgressRange,
) -> i32;
```

需要注意 `TransferRoots` 不是一个 `const` 函数，所以我们需要传递 `Pin<&mut STEPControl_Reader>` 类作为参数。

最后是 `reader.OneShape()` 函数：

```c++
Standard_EXPORT TopoDS_Shape OneShape() const;
```

不幸的是，这个函数返回一个裸 `TopoDS_Shape` 类，所以我们不能直接绑定此函数，我们需要创建一个 C++ 包装函数。

```rust
pub fn one_shape_step(reader: &STEPControl_Reader) -> UniquePtr<TopoDS_Shape>;
```

与

```c++
inline std::unique_ptr<TopoDS_Shape> one_shape_step(const STEPControl_Reader &reader) {
  return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(reader.OneShape()));
}
```

### 第 5 步 - 在 `build.rs` 中链接到 OpenCascade 库

OpenCascade 由许多类似的库组成，又是当你第一次引入一个全新的功能，你可能需要链接一个全新的库。否则，当你尝试基于这份新的代码构建一个二进制库时你会得到大量的链接错误。

在这个添加 STEP 文件支持的例子中，我们需要链接五个（！）不同的库。我们需要把他们添加到 `build.rs` 中。

```rust
println!("cargo:rustc-link-lib=static=TKSTEP");
println!("cargo:rustc-link-lib=static=TKSTEPAttr");
println!("cargo:rustc-link-lib=static=TKSTEPBase");
println!("cargo:rustc-link-lib=static=TKSTEP209");
println!("cargo:rustc-link-lib=static=TKXSBase");
```

你该怎样知道应该链接那些库？如果你搜索 `STEPControl_Reader` 你可能会来到 [这个页面](https://dev.opencascade.org/doc/refman/html/class_s_t_e_p_control___reader.html)。你可以在这里找到你需要的库名。

![TKSTEP](./images/cascade_library.png)

其余的库是通过反复试验发现的。链接器会抱怨缺少符号，所以将这些符号复制粘贴到搜索引擎中，在OpenCascade的文档站点上找到它们，并注意它们来自哪个库。或者，您可以通过项目的CMake和其他构建文件来确定需要链接到哪些库。

### 第 6 步 - 编写一个更好的 Rust API

终于，我们可以创建一个更高级的 Rust 函数用于读取一个 STEP 文件并且返回一个 `Shape` 图元：

```rust
pub fn from_step_file<P: AsRef<Path>>(path: P) -> Shape {
    let mut reader = STEPControl_Reader_ctor();
    let _return_status =
        read_step(reader.pin_mut(), path.as_ref().to_string_lossy().to_string());
    reader.pin_mut().TransferRoots(&Message_ProgressRange_ctor());

    let inner = one_shape_step(&reader);

    // Assuming a Shape struct has a UniquePtr<TopoDS_Shape> field called `inner`
    Shape { inner }
}
```

当然，一个更好的版本还可以优化 STEP 文件处理流程或返回 `Result` 泛型以用来处理可能的错误。

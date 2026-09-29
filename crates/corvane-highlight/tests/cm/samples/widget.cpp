// widget.cpp — C++20 sample for the clike port (çà et là, ∑ symbols)
#include <algorithm>
#include <memory>
#include <string_view>
#include <vector>
#include "widget.hpp"
#define STRINGIFY(x) #x
#define JOIN(a, b) a##b \
	/* continued */ + 1

namespace ui {
namespace detail {

constexpr std::size_t kMaxChildren = 1'000'000;
constexpr auto kMask = 0xFF'FF'00'00u;
constexpr double kPi = 3.141'592'653, kTiny = 1e-12, kHalf = .5;
constexpr auto kBin = 0b1010'0101ULL;
inline constexpr float kScale = 2.5f;

}  // namespace detail

template <typename T, std::size_t N = 4>
class SmallVec final : public Base<T>, private NonCopyable {
public:
	using value_type = T;
	SmallVec() noexcept = default;
	explicit SmallVec(std::initializer_list<T> init);
	~SmallVec() override;

	T &operator[](std::size_t i) { return data_[i]; }
	const T *begin() const { return data_; }
	[[nodiscard]] bool empty() const noexcept { return size_ == 0; }

private:
	T data_[N];
	std::size_t size_ = 0;
	mutable int cache_ = -1;
};

Widget::Widget(std::string name) : name_(std::move(name)) {}
Widget::~Widget() {}

void Widget::paint(Canvas &canvas) const
{
	auto raw = R"(C:\path\to\file "quoted" (parens))";
	auto delim = R"xyz(a )" still inside )xyz";
	auto multi = R"sql(
		SELECT * FROM widgets
		WHERE id = ?;
	)sql";
	auto u8s = u8"utf-8 ☃", u16 = u"utf-16", u32 = U"utf-32", wide = L"wide";
	char16_t ch = u'x';
	wchar_t wc = L'w';
	auto notRaw = Rect{0, 0, 10, 10};
	auto uRx = uRx_value + LRy + u8x;
	std::string s = "tab\t\"escaped\" and \\ backslash";
	char c = '\'';

	for (const auto &child : children_) {
		if (!child->visible()) continue;
		child->paint(canvas);
	}
	auto lambda = [this, &canvas](int x) mutable -> int { return x * 2; };
	std::sort(children_.begin(), children_.end(),
		[](const auto &a, const auto &b) { return a->z() < b->z(); });
	int *ptr = nullptr;
	int **pptr = &ptr;
	unsigned int *const cp = nullptr;
	static_assert(sizeof(int) >= 4, "int too small");
	auto p = std::make_unique<Widget>("child");
	auto casted = static_cast<long>(3.0) + reinterpret_cast<std::uintptr_t>(ptr);
	if (a and b or not c) { x = a xor b; }
	try {
		throw std::runtime_error("boom");
	} catch (const std::exception &e) {
		log(e.what());
	}
	co_await something();
	decltype(auto) value = get();
	return;
}

template <class F>
concept Callable = requires(F f) { f(); };

struct Point { int x, y; };
enum class Direction : std::uint8_t { North, East, South, West };
union Bits { std::uint32_t u; float f; };

std::vector<int> make_vector(int n);
Widget *find_widget(std::string_view id);
int main(int argc, char *argv[])
{
	ui::Widget w("root");
	w.paint(ui::Canvas::screen());
	auto s = STRINGIFY(hello);
	x = y*z;
	int unterminated = "oops;
	return __builtin_expect(argc, 1) ? _Exit(0), 0 : 1;
}

}  // namespace ui
auto broken = R"x(never closed
still in the raw string
done)x";

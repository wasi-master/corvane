#pragma once
#include <string>

namespace ui {

class Canvas;

/// A node in the widget tree.
class Widget {
public:
	explicit Widget(std::string name);
	virtual ~Widget();
	virtual void paint(Canvas &canvas) const;
	static Widget *root();
	int z() const { return z_; }
	bool visible() const noexcept { return visible_; }

protected:
	std::string name_;
	int z_ = 0;
	bool visible_ = true;
	friend class Layout;
};

inline Widget::Widget(std::string name) : name_(name) {}

template <typename T>
T clamp(T v, T lo, T hi) { return v < lo ? lo : hi < v ? hi : v; }

}  // namespace ui

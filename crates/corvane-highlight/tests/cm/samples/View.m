//
//  View.m — Objective-C sample (Ünïcödé: café, 東京)
//
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import "View.h"
#define kPadding 8.0f
#define SQUARE(x) ((x) * (x)) \
	/* continued */

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSInteger, ViewState) {
	ViewStateIdle = 0,
	ViewStateLoading = 1 << 0,
	ViewStateError = 0x10,
};

@protocol ViewDelegate <NSObject>
@required
- (void)viewDidLoad:(UIView *)view;
@optional
- (BOOL)shouldReload;
@end

@interface View : UIView <ViewDelegate>
@property (nonatomic, strong, nullable) NSString *title;
@property (atomic, readonly, copy) NSArray<NSNumber *> *values;
@property (nonatomic, weak) id<ViewDelegate> delegate;
@property (nonatomic, assign) BOOL enabled;
+ (instancetype)viewWithTitle:(NSString *)title;
@end

@implementation View {
	int _count;
	SEL _action;
	Class _cls;
}

@synthesize title = _title;
@dynamic values;

+ (instancetype)viewWithTitle:(NSString *)title
{
	View *view = [[self alloc] initWithFrame:CGRectZero];
	view.title = title;
	return view;
}

- (instancetype)initWithFrame:(CGRect)frame
{
	if ((self = [super initWithFrame:frame])) {
		_count = 0;
		_action = @selector(reload:);
		self.enabled = YES;
	}
	return self;
}

- (void)reload:(id)sender
{
	NSString *s = @"literal \"string\"";
	NSNumber *n = @42, *f = @3.5f;
	NSArray *items = @[@"a", @"b", @1];
	NSDictionary *map = @{@"key": @"value", @"n": @(SQUARE(3))};
	char c = 'x';
	unsigned long long big = 123456789ULL;
	double tiny = 1e-10, half = .5;
	__block int counter = 0;
	void (^block)(int) = ^(int x) { counter += x; };
	block(2);
	@try {
		[self doSomething:nil withValue:NO];
	} @catch (NSException *e) {
		NSLog(@"error: %@", e.reason);
	} @finally {
		counter--;
	}
	@synchronized (self) {
		_count++;
	}
	@autoreleasepool {
		for (NSString *item in items) {
			if (![item isKindOfClass:[NSString class]]) continue;
		}
	}
	if (@available(iOS 13, *)) { }
	id obj = nil; Class k = Nil; BOOL flag = NO;
	NSString *broken = @"unterminated
	switch (_count) {
		case 0: break;
		default: break;
	}
}

@end

NS_ASSUME_NONNULL_END

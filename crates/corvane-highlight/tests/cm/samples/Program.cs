// Program.cs — C# sample (àéîõü, Ελληνικά)
using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using static System.Console;

#nullable enable
#region Models

namespace Corvane.Sample
{
	/// <summary>
	/// A record with <see cref="Guid"/> ids.
	/// </summary>
	[Serializable, Obsolete("use V2")]
	public sealed record Person(string Name, int Age)
	{
		public Guid Id { get; init; } = Guid.NewGuid();
		public required string Email { get; set; }
	}

	internal struct Vector2 { public float X, Y; }

	public interface IRepository<T> where T : class
	{
		Task<IEnumerable<T>> AllAsync();
	}

	public static class Program
	{
		private const double Pi = 3.14159, Tiny = 1e-6, Half = .5;
		private static readonly int Hex = 0xFF, Bin = 0b1010, Big = 1_000_000;
		private static long L = 42L; static ulong U = 42UL; static float F = 1.5f; static decimal M = 9.99m;
		static char C = 'c', Nl = '\n', Q = '\'';

		public static async Task<int> Main(string[] args)
		{
			var path = @"C:\Users\""quoted""\file.txt";
			var multi = @"first line
second line with ""escape""
end";
			var name = "World";
			var greeting = $"Hello, {name}! Today is {DateTime.Now:yyyy-MM-dd}.";
			var both = $@"C:\{name}\dir";
			string escaped = "tab\t\"quote\" \\ backslash";
			int? maybe = null;
			object o = maybe ?? 0;
			var people = new List<Person> { new("Ada", 36) { Email = "ada@example.com" } };
			var adults = from p in people
				where p.Age >= 18
				orderby p.Name descending
				select p.Name;
			Func<int, int> square = x => x * x;
			Action<string> print = s => WriteLine(s);
			foreach (var person in people)
			{
				if (person is { Age: > 30 } older && !string.IsNullOrEmpty(older.Name))
					print(older.Name);
				else
					continue;
			}
			switch (args.Length)
			{
				case 0:
					break;
				default:
					goto case 0;
			}
			using (var stream = new System.IO.MemoryStream())
			{
				lock (stream) { checked { L += 1; } }
			}
			try
			{
				await Task.Delay(10);
				throw new InvalidOperationException("nope");
			}
			catch (Exception ex) when (ex.Message != "")
			{
				WriteLine(ex);
			}
			finally
			{
				GC.Collect();
			}
			unsafe
			{
				int* ptr = stackalloc int[4];
				fixed (char* cp = "abc") { }
			}
			var @class = typeof(Program);
			dynamic d = default(int);
			string bad = "unterminated
			return sizeof(int) + (args?.Length ?? 0);
		}

		public static IEnumerable<int> Evens(int max)
		{
			for (var i = 0; i < max; i += 2) yield return i;
		}

		public static T Identity<T>(T value) => value;
		event EventHandler? Changed;
		public static implicit operator string(Vector2 v) => $"{v.X},{v.Y}";
	}
}

#endregion

<%@ Page Language="C#" AutoEventWireup="true" CodeBehind="Default.aspx.cs" Inherits="WebApp._Default" %>
<%@ Register TagPrefix="uc" TagName="Header" Src="~/Controls/Header.ascx" %>
<%-- Server-side comment: <b>not</b> rendered, "quotes" & entities &amp; --%>
<%--
    A multi-line server comment
    with <% code %> inside that stays a comment
--%>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head runat="server">
	<title><%: Page.Title %> — Überblick</title>
	<link href="<%= ResolveUrl("~/Content/site.css") %>" rel="stylesheet" type="text/css" />
	<style>
		.grid td { padding: <%= Padding %>px; border: 1px solid #ccc; }
		/* css comment <%-- server comment in css --%> after */
	</style>
	<script type="text/javascript">
		var userId = <%= CurrentUser.Id %>;
		var name = "<%: CurrentUser.Name %>";
		function confirmDelete(id) {
			return confirm("Delete item " + id + "?");
		}
	</script>
</head>
<body>
	<form id="form1" runat="server">
		<uc:Header ID="Header1" runat="server" />
		<asp:Label ID="lblMessage" runat="server" Text="Hello" CssClass="msg"></asp:Label>
		<% if (User.Identity.IsAuthenticated) { %>
			<p>Welcome back, <%= Server.HtmlEncode(User.Identity.Name) %>!</p>
		<% } else { %>
			<p>Please <a href="Login.aspx?ReturnUrl=<%= Request.RawUrl %>">log in</a>.</p>
		<% } %>
		<asp:GridView ID="gvItems" runat="server" AutoGenerateColumns="false">
			<Columns>
				<asp:BoundField DataField="Name" HeaderText="Name" />
				<asp:TemplateField>
					<ItemTemplate><%# Eval("Price", "{0:C}") %></ItemTemplate>
				</asp:TemplateField>
			</Columns>
		</asp:GridView>
		<%
			// a multi-line code block
			var items = new List<string> { "a", "b" };
			foreach (var item in items) {
				Response.Write("<li>" + item + "</li>");
			}
		%>
		<p title="<%= "attr %> with close in string" %>">tricky</p>
		<p>100% sure, 50 % done, a < b and <%-- inline comment --%> after</p>
		<%$ Resources:Strings, Welcome %>
		<p>Unterminated server comment follows</p>
	</form>
</body>
</html>
<%-- unclosed comment at the end
still comment

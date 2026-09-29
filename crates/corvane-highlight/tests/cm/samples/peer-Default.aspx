<%@ Page Language="C#" AutoEventWireup="true" CodeBehind="Default.aspx.cs" Inherits="WebApp._Default" %>
<%-- A server-side comment
     spanning lines with <b>markup</b> --%>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head runat="server">
    <title><%= Page.Title %> - My App</title>
    <link href="~/Content/Site.css" rel="stylesheet" />
    <style>
        .grid td { padding: 4px 8px; border: 1px solid #ccc; }
    </style>
    <script type="text/javascript">
        function confirmDelete(id) {
            return confirm("Delete item " + id + "? <%= Resources.Confirm %>");
        }
    </script>
</head>
<body>
    <form id="form1" runat="server">
        <asp:Label ID="lblMessage" runat="server" Text="<%# Eval("Message") %>" CssClass="msg" />
        <% if (User.Identity.IsAuthenticated) { %>
            <p>Welcome back, <%: User.Identity.Name %>! 👋</p>
        <% } else { %>
            <p><a href="Login.aspx">Sign in</a></p>
        <% } %>
        <asp:GridView ID="grid" runat="server" AutoGenerateColumns="false">
            <Columns>
                <asp:BoundField DataField="Name" HeaderText="Name" />
            </Columns>
        </asp:GridView>
        <%
            var items = new List<string> { "a", "b" };
            foreach (var item in items) {
                Response.Write("<li>" + item + "</li>");
            }
        %>
        <p>Inline <%-- comment --%> and <% /* code */ %> on one line.</p>
        <%-- unterminated server comment
    </form>
</body>
</html>

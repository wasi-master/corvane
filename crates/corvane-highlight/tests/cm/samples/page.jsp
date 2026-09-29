<%@ page contentType="text/html;charset=UTF-8" language="java" import="java.util.*, com.example.model.User" %>
<%@ taglib prefix="c" uri="http://java.sun.com/jsp/jstl/core" %>
<%@ include file="/WEB-INF/jspf/header.jspf" %>
<%-- JSP comment: not sent to the client --%>
<%!
    private static final int MAX = 10;
    private String greet(String name) { return "Hello, " + name + "!"; }
%>
<!DOCTYPE html>
<html>
<head>
    <title>Users — <%= application.getAttribute("siteName") %></title>
    <script>
        var contextPath = "<%= request.getContextPath() %>";
        var max = <%= MAX %>;
        if (max > 5 && contextPath !== "") { console.log(`ok ${max}`); }
    </script>
    <style>body { font-family: "Noto Sans", sans-serif; }</style>
</head>
<body>
<%
    List<User> users = (List<User>) request.getAttribute("users");
    if (users == null) {
        users = new ArrayList<>();
    }
    String q = request.getParameter("q"); // may be null
%>
<h1><%= greet(session.getAttribute("name").toString()) %></h1>
<form action="search.jsp" method="get">
    <input type="text" name="q" value="<%= q == null ? "" : q %>">
    <button type="submit">Search</button>
</form>
<c:if test="${not empty users}">
    <ul>
    <c:forEach var="u" items="${users}" varStatus="st">
        <li class="${st.index % 2 == 0 ? 'even' : 'odd'}">${u.name} &lt;${u.email}&gt;</li>
    </c:forEach>
    </ul>
</c:if>
<% for (int i = 0; i < MAX; i++) { %>
    <span data-i="<%= i %>"><%= i * i %></span>
<% } %>
<jsp:include page="/WEB-INF/jspf/footer.jsp">
    <jsp:param name="year" value="2026" />
</jsp:include>
<p>Emoji 🎉 and tabs	here, 50% off, <%-- one --%><%-- two --%> done.</p>
<% String unclosed = "block that never closes";
   out.println(unclosed);

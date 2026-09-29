<%@ page contentType="text/html;charset=UTF-8" language="java" %>
<%@ taglib prefix="c" uri="http://java.sun.com/jsp/jstl/core" %>
<html>
<head>
  <title>${pageTitle}</title>
  <script>var ctx = "<%= request.getContextPath() %>";</script>
</head>
<body>
  <%-- JSP comment --%>
  <h1>Hello, <%= request.getParameter("name") %>!</h1>
  <c:forEach var="item" items="${items}">
    <div class="item">${item.name} &mdash; <fmt:formatNumber value="${item.price}" type="currency"/></div>
  </c:forEach>
  <%! private int counter = 0; %>
  <%
    counter++;
    String msg = counter > 1 ? "again" : "first time";
  %>
  <p>Visit number <%= counter %> (<%= msg %>)</p>
  <jsp:include page="footer.jsp" />
</body>
</html>
